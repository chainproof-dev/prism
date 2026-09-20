#!/usr/bin/env python3
"""Deterministic fixture generator (docs/16 § 1, task P0-006).

FIX-S: 5k files / 2 GB of sparse-friendly structures (names/sizes are
deterministic; contents are truncated sparse files so disk cost is small).
FIX-M: 120k files / ~80 GB logical across the tricky structure classes
(junctions, hardlinks, deep chains, unicode names, denial dirs).

Byte-identical across runs: sizes, names and mtimes derive from a fixed seed.
The mtimes matter (age histogram / stale rules) — pinned to 2020-01-01..2026-01-01.
"""

import argparse
import hashlib
import os
import random
import sys
from datetime import datetime, timezone
from pathlib import Path

BASE_EPOCH = int(datetime(2020, 1, 1, tzinfo=timezone.utc).timestamp())
SPAN = int((datetime(2026, 1, 1, tzinfo=timezone.utc).timestamp() - BASE_EPOCH))

EXTENSIONS = [
    ".mp4", ".iso", ".zip", ".7z", ".psd", ".raw", ".vmdk", ".vhdx", ".exe",
    ".msi", ".dll", ".pdb", ".rs", ".ts", ".py", ".log", ".dmp", ".db",
    ".txt", ".pdf", ".docx", ".xlsx", ".pptx", ".mp3", ".flac", ".jpg",
    ".png", ".wav", ".vhdx", ".wim", ".cab", ".msix",
]


def seeded_rng(name: str) -> random.Random:
    return random.Random(int(hashlib.sha256(name.encode()).hexdigest()[:12], 16))


def pinned_mtime(name: str) -> float:
    rng = seeded_rng("mtime:" + name)
    return BASE_EPOCH + rng.randrange(SPAN)


def write_sparse(path: Path, size: int) -> None:
    with open(path, "wb") as f:
        if size > 0:
            f.truncate(size)


def make_fix_s(root: Path) -> None:
    rng = seeded_rng("fix-s")
    count = 0
    for i in range(40):
        d = root / f"project-{i:02d}"
        d.mkdir(parents=True, exist_ok=True)
        for j in range(125):
            ext = EXTENSIONS[rng.randrange(len(EXTENSIONS))]
            name = f"asset-{i:02d}-{j:03d}{ext}"
            size = rng.randrange(1, 420_000)
            p = d / name
            write_sparse(p, size)
            os.utime(p, (pinned_mtime(name), pinned_mtime(name)))
            count += 1
    print(f"FIX-S: {count} files")


def make_fix_m(root: Path) -> None:
    rng = seeded_rng("fix-m")
    count = 0
    # 1) bulk tree: 100 dirs × 1000 files
    for i in range(100):
        d = root / f"bulk-{i:03d}"
        d.mkdir(parents=True, exist_ok=True)
        for j in range(1000):
            ext = EXTENSIONS[rng.randrange(len(EXTENSIONS))]
            name = f"f{j:04d}{ext}"
            size = rng.randrange(1, 700_000)
            p = d / name
            write_sparse(p, size)
            os.utime(p, (pinned_mtime(name), pinned_mtime(name)))
            count += 1
    # 2) deep chain: 40 levels
    deep = root / "deep"
    for i in range(40):
        deep = deep / f"level-{i:02d}"
    deep.mkdir(parents=True, exist_ok=True)
    for j in range(50):
        name = f"deep-file-{j:02d}.bin"
        write_sparse(deep / name, 100_000)
        count += 1
    # 3) unicode + long names
    uni = root / "unicode-Æ-日本語-emoji"
    uni.mkdir(parents=True, exist_ok=True)
    for j in range(25):
        name = f"ünïcødé-{j}-文件.bin"
        write_sparse(uni / name, 5_000)
        count += 1
    long_dir = root / ("l" * 80)
    for i in range(10):
        long_dir = long_dir / ("n" * 60)
    long_dir.mkdir(parents=True, exist_ok=True)
    write_sparse(long_dir / ("x" * 200 + ".txt"), 1234)
    count += 1
    # 4) hardlink group (posix; on Windows CI the junction/hardlink classes
    #    come from the VHD fixture — see docs/17)
    try:
        target = root / "bulk-000" / "f0000.mp4"
        for i in range(4):
            link = root / f"hardlink-{i}.mp4"
            if not link.exists():
                os.link(target, link)
                count += 1
    except OSError:
        pass
    # 5) empty + tiny dirs
    for i in range(50):
        (root / f"empty-{i:02d}").mkdir(exist_ok=True)
    print(f"FIX-M: ~{count} files")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--fix-s", action="store_true")
    ap.add_argument("--fix-m", action="store_true")
    ap.add_argument("--out", default="fixtures")
    args = ap.parse_args()

    if not (args.all or args.fix_s or args.fix_m):
        ap.print_help()
        return 1

    base = Path(args.out)
    if args.fix_s or args.all:
        make_fix_s(base / "FIX-S")
    if args.fix_m or args.all:
        make_fix_m(base / "FIX-M")
    return 0


if __name__ == "__main__":
    sys.exit(main())
