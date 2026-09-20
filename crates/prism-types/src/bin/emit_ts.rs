//! `emit-ts` — TypeScript contract emitter (docs/05-IPC-PROTOCOL.md § 6).
//!
//! Parses `prism-types`' own sources with `syn` and generates:
//!   - `protocol.ts`  every DTO as TS types (serde attrs respected)
//!   - `schemas.ts`   zod schemas for every DTO (T1 validation, PRISM-IPC-050)
//!   - `commands.ts`  `CommandsMap`, command names, premium map
//!   - `events.ts`    `EventsMap` + `EngineEvent` discriminated union
//!
//! `--check` mode compares against existing output and exits non-zero on
//! drift (CI gate). Hand edits to generated files are forbidden (PRISM-IPC-001).
//! Wire conventions per AMM-003: camelCase fields, kebab enum values,
//! u64 sizes cross as JSON numbers (exact ≤ 2^53 = 9 PB, 90× the largest
//! consumer volume on the market).

#![deny(missing_docs)]
#![allow(clippy::unwrap_used, clippy::expect_used)] // bin tool; panics surface as build errors

use std::collections::BTreeMap;
use std::path::PathBuf;
use syn::{Expr, Field, File, Item, ItemConst, ItemEnum, ItemStruct, ItemType, Lit};

const MODULES: &[&str] = &[
    include_str!("../ids.rs"),
    include_str!("../sys.rs"),
    include_str!("../scan.rs"),
    include_str!("../viz.rs"),
    include_str!("../types_list.rs"),
    include_str!("../filter.rs"),
    include_str!("../licensing.rs"),
    include_str!("../commands.rs"),
    include_str!("../events.rs"),
];

const HEADER: &str = "// GENERATED from crates/prism-types — DO NOT EDIT (PRISM-IPC-001).\n// Regenerate: cargo run -p prism-types --features emit -- emit-ts <dir>\n\n";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.first().map(String::as_str) == Some("--check");
    if check {
        args.remove(0);
    }
    let out_dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../packages/shared/src/generated"));
    let files = generate();
    let mut drifted = Vec::new();
    for (name, content) in &files {
        let path = out_dir.join(name);
        let up_to_date = std::fs::read_to_string(&path).is_ok_and(|existing| &existing == content);
        if up_to_date {
            continue;
        }
        if check {
            drifted.push(name.clone());
        } else {
            std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| panic!("mkdir {out_dir:?}: {e}"));
            std::fs::write(&path, content).unwrap_or_else(|e| panic!("write {path:?}: {e}"));
            println!("wrote {}", path.display());
        }
    }
    if check {
        if drifted.is_empty() {
            println!("codegen: in sync ({} files)", files.len());
        } else {
            eprintln!("codegen DRIFT detected in: {}", drifted.join(", "));
            eprintln!("run `cargo run -p prism-types --features emit -- emit-ts <dir>` and commit");
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// model
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Model {
    aliases: BTreeMap<String, String>,
    structs: BTreeMap<String, Vec<TsField>>,
    /// enum name → (optional serde tag, variants)
    enums: BTreeMap<String, (Option<String>, Vec<(String, Vec<TsField>)>)>,
    commands: Vec<CmdDef>,
}

#[derive(Clone)]
struct TsField {
    name: String,
    ty: String,
    optional: bool,
}

struct CmdDef {
    cmd: String,
    req: String,
    res: String,
    premium: Option<String>,
}

fn generate() -> Vec<(String, String)> {
    let mut model = Model::default();
    for src in MODULES {
        parse_file(src, &mut model);
    }
    vec![
        ("protocol.ts".to_string(), emit_protocol(&model)),
        ("schemas.ts".to_string(), emit_schemas(&model)),
        ("commands.ts".to_string(), emit_commands(&model)),
        ("events.ts".to_string(), emit_events(&model)),
    ]
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

fn parse_file(src: &str, model: &mut Model) {
    let file: File = syn::parse_str(src).unwrap_or_else(|e| panic!("parse: {e}"));
    for item in file.items {
        match item {
            Item::Type(ItemType { ident, ty, .. }) => {
                let (ty, _) = type_expr_opt(&ty);
                model.aliases.insert(ident.to_string(), ty);
            }
            Item::Struct(ItemStruct { ident, fields, .. }) => {
                if let syn::Fields::Named(named) = fields {
                    model.structs.insert(
                        ident.to_string(),
                        named.named.iter().map(field_ts).collect(),
                    );
                }
            }
            Item::Enum(ItemEnum {
                ident,
                variants,
                attrs,
                ..
            }) => {
                let rename_all =
                    serde_str_attr(&attrs, "rename_all").unwrap_or_else(|| "camelCase".into());
                let tag = serde_str_attr(&attrs, "tag");
                let mut out = Vec::new();
                for v in variants {
                    let mut fields = Vec::new();
                    if let syn::Fields::Named(named) = &v.fields {
                        for f in named.named.iter() {
                            fields.push(field_ts(f));
                        }
                    }
                    out.push((wire_name(&v.ident.to_string(), &rename_all), fields));
                }
                model.enums.insert(ident.to_string(), (tag, out));
            }
            Item::Const(ItemConst { ident, expr, .. }) if ident == "COMMANDS" => {
                parse_commands(&expr, model);
            }
            _ => {}
        }
    }
}

/// Extract a string literal from an expression (None otherwise).
fn lit_str(e: &Expr) -> Option<String> {
    if let Expr::Lit(inner) = e {
        if let Lit::Str(s) = &inner.lit {
            return Some(s.value());
        }
    }
    None
}

fn parse_commands(expr: &Expr, model: &mut Model) {
    let Expr::Reference(r) = expr else { return };
    let Expr::Array(arr) = &*r.expr else { return };
    for elem in arr.elems.iter() {
        let Expr::Struct(init) = elem else { continue };
        let mut cmd = String::new();
        let mut req = String::new();
        let mut res = String::new();
        let mut premium = None;
        for f in init.fields.iter() {
            let member = match &f.member {
                syn::Member::Named(m) => m.to_string(),
                syn::Member::Unnamed(_) => continue,
            };
            match (lit_str(&f.expr), member.as_str()) {
                (Some(v), "cmd") => cmd = v,
                (Some(v), "req") => req = v,
                (Some(v), "res") => res = v,
                (_, "premium") => {
                    if let Expr::Call(c) = &f.expr {
                        if let Some(arg) = c.args.first() {
                            premium = lit_str(arg);
                        }
                    }
                }
                _ => {}
            }
        }
        model.commands.push(CmdDef {
            cmd,
            req,
            res,
            premium,
        });
    }
}

/// serde attributes we honor: `rename_all`, `tag` (both as string values).
fn serde_str_attr(attrs: &[syn::Attribute], key: &str) -> Option<String> {
    for a in attrs {
        if !a.path().is_ident("serde") {
            continue;
        }
        let Ok(list) = a.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        ) else {
            continue;
        };
        for m in list {
            if let syn::Meta::NameValue(nv) = m {
                if nv.path.is_ident(key) {
                    if let Some(v) = lit_str(&nv.value) {
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}

fn field_ts(f: &Field) -> TsField {
    let raw = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
    let (ty, optional) = type_expr_opt(&f.ty);
    TsField {
        name: snake_to_camel(&raw),
        ty,
        optional,
    }
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper = false;
    for ch in s.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn wire_name(name: &str, rename: &str) -> String {
    match rename {
        "kebab-case" => to_snake(name).replace('_', "-"),
        "camelCase" => snake_to_camel(&to_snake(name)),
        _ => to_snake(name),
    }
}

/// TS type expression + optionality for a Rust type reference.
fn type_expr_opt(ty: &syn::Type) -> (String, bool) {
    use syn::Type as T;
    match ty {
        T::Path(p) => {
            let last = p.path.segments.last();
            let ident = last.map(|s| s.ident.to_string()).unwrap_or_default();
            let args: Vec<String> = last
                .and_then(|s| match &s.arguments {
                    syn::PathArguments::AngleBracketed(ab) => Some(
                        ab.args
                            .iter()
                            .filter_map(|a| match a {
                                syn::GenericArgument::Type(t) => {
                                    let (t, _) = type_expr_opt(t);
                                    Some(t)
                                }
                                _ => None,
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            match ident.as_str() {
                "Option" => (
                    args.into_iter().next().unwrap_or_else(|| "unknown".into()),
                    true,
                ),
                "Vec" => {
                    let e = args.first().cloned().unwrap_or_else(|| "unknown".into());
                    (format!("{e}[]"), false)
                }
                "HashMap" => (
                    format!(
                        "Record<{}, {}>",
                        args.first().cloned().unwrap_or_else(|| "string".into()),
                        args.get(1).cloned().unwrap_or_else(|| "unknown".into())
                    ),
                    false,
                ),
                _ => (map_ident(&ident), false),
            }
        }
        T::Reference(r) => type_expr_opt(&r.elem),
        _ => ("unknown".into(), false),
    }
}

fn map_ident(ident: &str) -> String {
    match ident {
        "String" | "PathBuf" => "string".into(),
        "bool" => "boolean".into(),
        // byte counts cross as BigInt (docs/05 § 3.3 — exact end-to-end)
        "u64" | "i64" | "u128" => "bigint".into(),
        "u8" | "u16" | "u32" | "i8" | "i16" | "i32" | "usize" | "isize" | "f32" | "f64" => "number".into(),
        _ => ident.to_string(),
    }
}

// ---------------------------------------------------------------------------
// emission: protocol.ts
// ---------------------------------------------------------------------------

fn emit_protocol(m: &Model) -> String {
    let mut out = String::from(HEADER);
    out.push_str("// Wire conventions (AMM-003): camelCase fields, kebab enum values,\n// u64 byte counts as JSON numbers (exact ≤ 2^53).\n\n");
    for (name, target) in &m.aliases {
        if matches!(target.as_str(), "string" | "boolean" | "number") {
            out.push_str(&format!("export type {name} = {target};\n"));
        } else {
            out.push_str(&format!("export type {name} = {target};\n"));
        }
    }
    if !m.aliases.is_empty() {
        out.push('\n');
    }
    for (name, (tag, variants)) in &m.enums {
        if let Some(tag) = tag {
            out.push_str(&format!("export type {name} =\n"));
            for (i, (vname, fields)) in variants.iter().enumerate() {
                out.push_str(&format!("  | {{ {tag}: '{vname}'"));
                if fields.is_empty() {
                    out.push_str(" }\n");
                } else {
                    out.push_str("; ");
                    for (j, f) in fields.iter().enumerate() {
                        if j > 0 {
                            out.push_str("; ");
                        }
                        let opt = if f.optional { "?" } else { "" };
                        out.push_str(&format!("{}{}: {}", f.name, opt, f.ty));
                    }
                    out.push_str(" }\n");
                }
                if i == variants.len() - 1 {
                    out.push_str("  ;\n");
                }
            }
        } else {
            let arms: Vec<String> = variants.iter().map(|(v, _)| format!("'{v}'")).collect();
            out.push_str(&format!("export type {name} = {};\n", arms.join(" | ")));
        }
        out.push('\n');
    }
    for (name, fields) in &m.structs {
        if fields.is_empty() {
            out.push_str(&format!("export interface {name} {{}}\n\n"));
            continue;
        }
        out.push_str(&format!("export interface {name} {{\n"));
        for f in fields {
            let opt = if f.optional { "?" } else { "" };
            out.push_str(&format!("  {}{}: {};\n", f.name, opt, f.ty));
        }
        out.push_str("}\n\n");
    }
    out
}

// ---------------------------------------------------------------------------
// emission: schemas.ts (zod)
// ---------------------------------------------------------------------------

fn emit_schemas(m: &Model) -> String {
    let mut out = String::from(HEADER);
    out.push_str("import { z } from 'zod';\n\n");
    for (name, (tag, variants)) in &m.enums {
        if let Some(tag) = tag {
            let mut parts = Vec::new();
            for (vname, fields) in variants {
                let mut body = format!("{tag}: z.literal('{vname}')");
                for f in fields {
                    let (key, sch) = zod_field(f);
                    body.push_str(&format!(", {key}: {sch}"));
                }
                parts.push(format!("z.object({{ {body} }})"));
            }
            out.push_str(&format!(
                "export const {name}Schema = z.discriminatedUnion('{tag}', [{}]);\n\n",
                parts.join(", ")
            ));
        } else {
            let arms: Vec<String> = variants.iter().map(|(v, _)| format!("'{v}'")).collect();
            out.push_str(&format!(
                "export const {name}Schema = z.enum([{}]);\n\n",
                arms.join(", ")
            ));
        }
    }
    for (name, fields) in &m.structs {
        let mut body = String::new();
        for f in fields {
            let (key, sch) = zod_field(f);
            body.push_str(&format!("  {key}: {sch},\n"));
        }
        if body.is_empty() {
            out.push_str(&format!("export const {name}Schema = z.object({{}});\n\n"));
        } else {
            out.push_str(&format!(
                "export const {name}Schema = z.object({{\n{body}}});\n\n"
            ));
        }
    }
    // aliases
    for (name, target) in &m.aliases {
        if let Some((enum_name, _)) = m.enums.get_key_value(target) {
            let _ = enum_name;
            out.push_str(&format!("export const {name}Schema = {target}Schema;\n"));
        } else if matches!(target.as_str(), "string" | "boolean" | "number") {
            let prim = match target.as_str() {
                "string" => "z.string()",
                "boolean" => "z.boolean()",
                _ => "z.number()",
            };
            out.push_str(&format!("export const {name}Schema = {prim};\n"));
        }
    }
    out
}

fn zod_field(f: &TsField) -> (String, String) {
    let base = zod_expr(&f.ty);
    let expr = if f.optional {
        match base.as_str() {
            "z.string()" | "z.boolean()" | "z.number()" => format!("{base}.optional()"),
            other => {
                if other.starts_with("z.lazy") {
                    format!("{other}.optional()")
                } else {
                    format!("{other}.optional()")
                }
            }
        }
    } else {
        base
    };
    (f.name.clone(), expr)
}

fn zod_expr(ty: &str) -> String {
    if let Some(elem) = ty.strip_suffix("[]") {
        let inner = zod_expr(elem);
        if inner.starts_with("z.lazy") {
            // arrays of lazy refs need the lazy outside
            return format!("z.lazy(() => z.array({}))", strip_lazy(&inner));
        }
        return format!("z.array({inner})");
    }
    if let Some(rest) = ty.strip_prefix("Record<string, ") {
        let val = rest.strip_suffix('>').unwrap_or(rest);
        return format!("z.record(z.string(), {})", zod_expr(val));
    }
    match ty {
        "string" => "z.string()".into(),
        "number" => "z.number()".into(),
        "bigint" => "z.bigint()".into(),
        "boolean" => "z.boolean()".into(),
        other => format!("z.lazy(() => {other}Schema)"),
    }
}

fn strip_lazy(s: &str) -> String {
    s.trim_start_matches("z.lazy(() => ")
        .trim_end_matches(')')
        .to_string()
}

// ---------------------------------------------------------------------------
// emission: commands.ts
// ---------------------------------------------------------------------------

fn emit_commands(m: &Model) -> String {
    let mut out = String::from(HEADER);
    out.push_str("import type * as P from './protocol';\n\n");
    out.push_str("export type CommandName =\n");
    for c in &m.commands {
        out.push_str(&format!("  | '{}'\n", c.cmd));
    }
    out.push_str("  ;\n\nexport interface CommandsMap {\n");
    for c in &m.commands {
        let req = if c.req == "()" {
            "null".to_string()
        } else {
            format!("P.{}", c.req)
        };
        let res = if c.res == "()" {
            "void".to_string()
        } else {
            format!("P.{}", c.res)
        };
        out.push_str(&format!("  '{}': {{ req: {req}; res: {res} }};\n", c.cmd));
    }
    out.push_str("}\n\n");
    // request schema lookup (T1 validation in main, PRISM-IPC-050)
    out.push_str("import * as S from './schemas';\n\nexport interface RequestSchemas {\n");
    for c in &m.commands {
        if c.req == "()" {
            out.push_str(&format!("  '{}': null;\n", c.cmd));
        } else {
            out.push_str(&format!("  '{}': S.{}Schema;\n", c.cmd, c.req));
        }
    }
    out.push_str("}\n\n");
    // premium gating map (docs/05 § 3.7)
    out.push_str("export const PremiumCommands: Record<string, P.PremiumFeature> = {\n");
    for c in &m.commands {
        if let Some(p) = &c.premium {
            out.push_str(&format!("  '{}': '{}',\n", c.cmd, p));
        }
    }
    out.push_str("};\n");
    out
}

// ---------------------------------------------------------------------------
// emission: events.ts
// ---------------------------------------------------------------------------

fn emit_events(m: &Model) -> String {
    let mut out = String::from(HEADER);
    out.push_str("import type * as P from './protocol';\n\n");
    // EngineEvent discriminated union (from the events module's tagged enum)
    if let Some((_, variants)) = m.enums.get("EngineEvent") {
        out.push_str("export type EngineEvent =\n");
        for (tag, fields) in variants {
            let payload = fields
                .first()
                .map(|f| format!("{}: P.{}", f.name, f.ty))
                .unwrap_or_else(|| "payload: unknown".into());
            out.push_str(&format!("  | {{ ev: '{tag}'; {payload} }}\n"));
        }
        out.push_str("  ;\n\n");
        out.push_str("export interface EventsMap {\n");
        for (tag, fields) in variants {
            let event = kebab_to_colon(tag);
            let payload_ty = fields
                .first()
                .map(|f| format!("P.{}", f.ty))
                .unwrap_or_else(|| "unknown".into());
            out.push_str(&format!("  '{event}': {{ payload: {payload_ty} }};\n"));
        }
        out.push_str("}\n\nexport type EventName = keyof EventsMap;\n");
    }
    out
}

/// 'scan-error-batch' → 'scan:error-batch' (first segment becomes the domain).
fn kebab_to_colon(s: &str) -> String {
    match s.split_once('-') {
        Some((head, rest)) => format!("{head}:{rest}"),
        None => s.to_string(),
    }
}
