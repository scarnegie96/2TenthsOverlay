#!/usr/bin/env python3
"""Summarise a heap profile written by `tenths --heap-profile` (dhat's JSON).

    python3 tools/heap_top.py tenths-heap.json [--top 15] [--stacks 3]

Each allocation is attributed to the first frame in our own code (tenths:: or telemetry::)
on its call stack, or failing that the first frame outside the standard library, so "where
the memory goes" reads as lines of this repo. Three tables:

  at peak   what was live when the heap was biggest (where the memory is)
  at end    what was still live when Tenths quit (what it holds on to)
  churn     everything ever allocated, with block counts (where the allocator is busy)

The same file opens in https://nnethercote.github.io/dh_view/dh_view.html as a tree.
Build for it with: cargo build --profile heap --features heap-profile (see app/src/heap.rs).
"""
import argparse
import json
import re
import sys
from collections import defaultdict

OURS = ("tenths::", "telemetry::", "<tenths::", "<telemetry::")
# Frames that are the allocator or the standard library plumbing, not who asked for memory.
NOISE = ("std::", "core::", "alloc::", "<alloc::", "<core::", "<std::", "hashbrown::", "<hashbrown::",
         "dhat::", "<dhat::", "__rust", "start_thread", "clone", "_start", "<T as", "<&")


def clean(frame: str) -> str:
    """'0xADDR: <T as X>::f::<A, B> (src/file.rs:1:2)' -> 'f (src/file.rs:1)'."""
    frame = re.sub(r"^0x[0-9a-f]+: ", "", frame)
    frame = re.sub(r":(\d+)\)$", ")", frame)
    for _ in range(8):  # generics, innermost first
        shorter = re.sub(r"::<[^<>]*>", "", frame)
        shorter = re.sub(r"<([^<>]*?) as [^<>]*>", r"\1", shorter)
        if shorter == frame:
            break
        frame = shorter
    return frame


def site_of(frames):
    """The frame that stands for this allocation: ours if any, else the first non-plumbing one."""
    for f in frames:
        if f.startswith(OURS):
            return f
    for f in frames:
        if not f.startswith(NOISE):
            return f
    return frames[0] if frames else "[unknown]"


def mb(n):
    return f"{n / 1048576:9.2f}"


def table(title, rows, key, top):
    total = sum(r[key] for r in rows.values())
    print(f"\n== {title}: {mb(total).strip()} MB in {sum(r[key + 'k'] for r in rows.values()):,} blocks")
    print(f"{'MB':>9} {'%':>5} {'blocks':>10}  site")
    shown = 0
    for site, r in sorted(rows.items(), key=lambda kv: -kv[1][key])[:top]:
        if r[key] == 0:
            break
        pct = 100 * r[key] / total if total else 0
        print(f"{mb(r[key])} {pct:5.1f} {r[key + 'k']:>10,}  {site}")
        shown += 1
    if not shown:
        print("  (nothing)")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file")
    ap.add_argument("--top", type=int, default=15, help="rows per table (default 15)")
    ap.add_argument("--stacks", type=int, default=0, help="also print the call stacks of the N biggest sites at the peak")
    args = ap.parse_args()

    d = json.load(open(args.file))
    if d.get("mode") != "rust-heap":
        sys.exit(f"{args.file}: not a dhat heap profile (mode {d.get('mode')!r})")
    ftbl = [clean(f) for f in d["ftbl"]]

    rows = defaultdict(lambda: defaultdict(int))
    stacks = []
    for pp in d["pps"]:
        frames = [ftbl[i] for i in pp["fs"] if ftbl[i] != "[root]"]
        site = site_of(frames)
        r = rows[site]
        for k in ("gb", "gbk", "eb", "ebk", "tb", "tbk"):
            r[k] += pp[k]
        stacks.append((pp["gb"], site, frames))

    secs = d["te"] / 1e6
    print(f"{args.file}: {secs:.1f} s, peak at {d['tg'] / 1e6:.1f} s")
    table("at peak", rows, "gb", args.top)
    table("at end", rows, "eb", args.top)
    table("churn (all allocations)", rows, "tb", args.top)

    for gb, site, frames in sorted(stacks, key=lambda s: -s[0])[: args.stacks]:
        print(f"\n-- {mb(gb).strip()} MB at peak: {site}")
        for f in frames:
            if f.startswith(NOISE) and not f.startswith(OURS):
                continue
            print(f"     {f}")


if __name__ == "__main__":
    main()
