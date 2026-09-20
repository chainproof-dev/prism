#!/usr/bin/env python3
"""ar shim: cc-rs on msvc targets invokes lib.exe-style args
('-out:ARCHIVE', '-nologo', members...). Translate to GNU ar. For cargo
check the archive is never linked — this exists so build scripts complete."""
import subprocess, sys
argv = sys.argv[1:]
out = None
members = []
for a in argv:
    low = a.lower()
    if low.startswith("-out:") or low.startswith("/out:"):
        out = a.split(":", 1)[1]
    elif a.startswith("@"):
        try:
            with open(a[1:]) as rf:
                members += rf.read().split()
        except OSError:
            pass
    elif a.startswith("-") or a.startswith("/"):
        continue
    else:
        members.append(a)
if out is None:
    out = members.pop(0) if members else "/dev/null"
sys.exit(subprocess.call(["ar", "rcs", out] + members))
