#!/usr/bin/env python3
"""CycloneDX SBOM for a release (docs/15 § 7-8): cargo + npm components.

Usage: python3 script/sbom.py --out dist/sbom.json
Requires `cargo metadata` and `pnpm list -r --json` on PATH.
"""
import argparse, json, subprocess, sys, datetime, uuid


def cargo_components() -> list[dict]:
    meta = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1"]))
    out = []
    for pkg in meta.get("packages", []):
        if pkg.get("source") is None:  # workspace member
            continue
        out.append({
            "type": "library",
            "bom-ref": f"cargo:{pkg['name']}@{pkg['version']}",
            "name": pkg["name"],
            "version": pkg["version"],
            "purl": f"pkg:cargo/{pkg['name']}@{pkg['version']}",
            "licenses": [{"license": {"id": l.get("id", "Unknown")}} for l in pkg.get("license", "").split(" OR ") if l] or None,
        })
    return out


def npm_components() -> list[dict]:
    try:
        listing = json.loads(subprocess.check_output(["pnpm", "list", "-r", "--json", "--depth", "0"]))
    except (OSError, subprocess.CalledProcessError):
        return []
    out = []
    for project in listing:
        for name, info in project.get("dependencies", {}).items():
            out.append({
                "type": "library",
                "bom-ref": f"npm:{name}@{info.get('version', '?')}",
                "name": name,
                "version": info.get("version", "?"),
                "purl": f"pkg:npm/{name}@{info.get('version', '?')}",
            })
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    doc = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid4()}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "component": {"type": "application", "name": "Prism", "version": "0.1.0"},
        },
        "components": cargo_components() + npm_components(),
    }
    with open(args.out, "w") as f:
        json.dump(doc, f, indent=2)
    print(f"SBOM: {args.out} ({len(doc['components'])} components)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
