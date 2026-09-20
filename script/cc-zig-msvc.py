#!/usr/bin/env python3
"""Cross C compiler shim for cargo build scripts validating the
x86_64-pc-windows-msvc *Rust* target on a Linux host (AMM-002).

The C side (libsqlite3-sys, zstd-sys bundled sources) is compiled with zig's
windows-gnu target because zig bundles MinGW headers but not MSVC UCRT ones.
This is sound for `cargo check`: build-script objects are never linked in a
check build — the validation target is the Rust cfg(windows) code, which
windows-sys binds for the MSVC ABI on the real target. The authoritative
compile remains the windows-latest CI job.
"""
import subprocess, sys, os

ZIG = os.path.expanduser("~/.local/zig/zig-x86_64-linux-0.14.1/zig")
out = []
i = 0
argv = sys.argv[1:]
while i < len(argv):
    a = argv[i]
    if a == "--target=x86_64-pc-windows-msvc":
        out += ["-target", "x86_64-windows-gnu"]; i += 1; continue
    if a == "--target" and i + 1 < len(argv) and argv[i+1] == "x86_64-pc-windows-msvc":
        out += ["-target", "x86_64-windows-gnu"]; i += 2; continue
    out.append(a); i += 1
sys.exit(subprocess.call([ZIG, "cc"] + out))
