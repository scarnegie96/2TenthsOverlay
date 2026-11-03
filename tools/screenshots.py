#!/usr/bin/env python3
"""Takes every README picture from the app itself, on the demo race.

    python3 tools/screenshots.py            # everything (about 15 minutes)
    python3 tools/screenshots.py overview   # just the three overviews
    python3 tools/screenshots.py widgets    # just docs/widgets/*.png
    python3 tools/screenshots.py settings   # just docs/settings/*.png

Needs Linux with Xvfb (`xvfb-run`), Python 3 with Pillow, NumPy and SciPy, and
a built app: `cargo build --profile fast` (or set TENTHS_BIN). The demo race
is fast-forwarded (`--warmup`) so every panel is full: laps in the lap log,
stints and stops, the race pace history, fuel averages.

The pictures are composited over the same stand-in for the game the old
browser preview used (a sky, the horizon, a darker track).
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
BIN = Path(os.environ.get("TENTHS_BIN", ROOT / "target" / "fast" / "tenths"))

# Demo race seconds to fast-forward, and when to take the picture: the
# player's lap is then about three quarters done (a full delta trace).
WARMUP = 555
SHOT_AFTER = 68

# Every widget on a 2560x1440 screen at 100%: the overview layout.
SHOWCASE = {
    "standings": (24, 24), "relative": (24, 604), "dynamics": (24, 846), "inputs": (24, 1302),
    "delta": (568, 24), "racepace": (568, 258), "session": (1132, 258),
    "coach": (568, 612), "flags": (990, 612), "fuel": (1170, 612),
    "laplog": (568, 836), "radar": (892, 836), "instruments": (760, 1213),
    "stints": (1468, 24), "tires": (1468, 716), "pitstop": (1772, 716), "weather": (2132, 24),
    "battle": (2132, 476),
}
ALL = list(SHOWCASE)
# Options the pictures need: the radar and flag board always shown.
OPTIONS = {
    "radar": {"alwaysShow": True},
    "flags": {"alwaysShow": True},
}


def config(widgets, theme="dark", scale=1.0, extra=None):
    """A config.json with `widgets` = {id: (x, y)} on and everything else off."""
    w = {}
    for wid in ALL:
        x, y = widgets.get(wid, (0, 0))
        opts = dict(OPTIONS.get(wid, {}))
        opts.update((extra or {}).get(wid, {}))
        w[wid] = {"enabled": wid in widgets, "x": x, "y": y, "ax": "l", "ay": "t", "scale": scale, "opacity": 1, "options": opts}
    return {
        "version": 1,
        "general": {"demo": True, "units": "metric", "showWhen": "connected", "theme": theme,
                    "colors": {"dark": {}, "light": {}, "f1": {}}, "activeProfile": None, "saveReports": False},
        "widgets": w,
    }


def run(cfg, out, size=(2560, 1440), args=(), env=None, warmup=WARMUP, shot_after=SHOT_AFTER, flag=16384):
    """Runs the app on the demo race and saves its window as `out` (transparent PNG)."""
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "config.json"
        path.write_text(json.dumps(cfg))
        data = Path(tmp) / "data"
        data.mkdir()
        cmd = ["xvfb-run", "-a", "-s", f"-screen 0 {size[0]}x{size[1]}x24", str(BIN), "--demo", "--config", str(path),
               "--data-dir", str(env.pop("DATA_DIR") if env and "DATA_DIR" in env else data),
               "--shot", str(out), "--shot-after", str(shot_after)]
        if warmup:
            cmd += ["--warmup", str(warmup)]
        # A safety car out on the flag board, as the old overview had (the demo only waves flags now and then).
        cmd += ["--flag", str(flag)]
        cmd += list(args)
        full_env = dict(os.environ, **(env or {}))
        subprocess.run(cmd, check=True, env=full_env, stdout=subprocess.DEVNULL, timeout=shot_after + 120)


def backdrop(w, h):
    """The stand-in for the game: `linear-gradient(170deg, …)` plus a darker bottom."""
    stops = [(0.0, (0x3B, 0x55, 0x68)), (0.38, (0x6F, 0x8A, 0x97)), (0.52, (0x8A, 0x8F, 0x7E)),
             (0.53, (0x4A, 0x4C, 0x43)), (1.0, (0x2E, 0x30, 0x2B))]
    a = np.deg2rad(170)
    dx, dy = np.sin(a), -np.cos(a)
    length = abs(w * dx) + abs(h * dy)
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    t = ((xs - w / 2) * dx + (ys - h / 2) * dy) / length + 0.5
    pos = np.array([s[0] for s in stops])
    img = np.stack([np.interp(t, pos, [s[1][c] for s in stops]) for c in range(3)], axis=-1)
    shade = np.clip((ys / h - 0.55) / 0.45, 0, 1)[..., None] * 0.35
    img = img * (1 - shade)
    return Image.fromarray(img.astype(np.uint8), "RGB").convert("RGBA")


def composite(shot, crop=None, bg_size=None):
    """The transparent screenshot over the backdrop, cropped to `crop` (l, t, r, b)."""
    im = Image.open(shot).convert("RGBA")
    bg = backdrop(*(bg_size or im.size))
    if bg.size != im.size:
        bg = bg.resize(im.size)
    bg.alpha_composite(im)
    return bg.crop(crop) if crop else bg


def save(img, name):
    path = DOCS / name
    path.parent.mkdir(parents=True, exist_ok=True)
    img.convert("RGB").save(path, optimize=True)
    print("  ", path.relative_to(ROOT))


def bbox(alpha, region):
    """Bounding box of everything drawn inside `region` (l, t, r, b)."""
    l, t, r, b = region
    mask = alpha[t:b, l:r] > 8
    ys, xs = np.nonzero(mask)
    if len(xs) == 0:
        return None
    return (l + xs.min(), t + ys.min(), l + xs.max() + 1, t + ys.max() + 1)


# --- The pictures ----------------------------------------------------------------

def overview(tmp):
    print("overviews")
    for theme, name in [("dark", "screenshot.png"), ("light", "screenshot-light.png"), ("f1", "screenshot-boxbox.png")]:
        out = tmp / f"overview-{theme}.png"
        run(config(SHOWCASE, theme), out)
        save(composite(out), name)


def batches(jobs, size=4):
    """Groups of up to `size` jobs, never the same widget twice in one group (one config per run)."""
    out = []
    for job in jobs:
        for b in out:
            if len(b) < size and all(j[1] != job[1] for j in b):
                b.append(job)
                break
        else:
            out.append([job])
    return out


def wheels(tmp):
    widgets(tmp, only_wheels=True)


def widgets(tmp, only_wheels=False):
    """Each widget at 200% (sharp on high-DPI screens), four to a run, one per quarter of the screen."""
    print("widgets" if not only_wheels else "wheels")
    W, H = 5120, 2880
    quarters = [(0, 0), (W // 2, 0), (0, H // 2), (W // 2, H // 2)]
    pad = 48
    jobs = [(wid, wid, {}) for wid in ALL]
    jobs += [(f"wheel-{w}", "inputs", {"inputs": {"wheel": w}}) for w in ("round", "gt", "formula", "oval")]
    if only_wheels:
        jobs = [j for j in jobs if j[0].startswith("wheel-")]
    # TENTHS_ONLY=inputs,fuel retakes just those pictures.
    only = [n for n in os.environ.get("TENTHS_ONLY", "").split(",") if n]
    if only:
        jobs = [j for j in jobs if j[0] in only]
    # Inputs: room above for the reference strip.
    top = {"inputs": 260}
    crops = {}
    for k, batch in enumerate(batches(jobs)):
        layout, extra = {}, {}
        for (name, wid, opts), (qx, qy) in zip(batch, quarters):
            layout[wid] = (qx + 100, qy + 100 + top.get(wid, 0))
            for i, o in opts.items():
                extra.setdefault(i, {}).update(o)
        cfg = config(layout, "dark", scale=2.0, extra=extra)
        out = tmp / f"widgets-{k}.png"
        run(cfg, out, size=(W, H))
        alpha = np.array(Image.open(out).convert("RGBA"))[..., 3]
        full = composite(out)
        for (name, wid, _), (qx, qy) in zip(batch, quarters):
            box = bbox(alpha, (qx, qy, qx + W // 2, qy + H // 2))
            if box is None:
                print("   nothing drawn for", name)
                continue
            l, t, r, b = box
            crops[name] = full.crop((max(l - pad, 0), max(t - pad, 0), r + pad, b + pad))
    if not only_wheels and (not only or "flags" in only):
        # The flag board under a blue flag, with the driver panel for the car behind.
        out = tmp / "flags-blue.png"
        run(config({"flags": (100, 100)}, "dark", scale=2.0, extra={"flags": {"alwaysShow": True}}), out, size=(W // 2, H // 2), flag=32)
        alpha = np.array(Image.open(out).convert("RGBA"))[..., 3]
        box = bbox(alpha, (0, 0, W // 2, H // 2))
        if box:
            l, t, r, b = box
            crops["flags"] = composite(out).crop((max(l - pad, 0), max(t - pad, 0), r + pad, b + pad))
    for wid in ([] if only_wheels else ALL):
        if wid in crops:
            save(crops[wid], f"widgets/{wid}.png")
    # The four steering wheels side by side.
    wheels = [crops.get(f"wheel-{w}") for w in ("round", "gt", "formula", "oval")]
    wheels = [w for w in wheels if w is not None]
    if wheels and not only:
        wsz = sum(w.width for w in wheels)
        sheet = Image.new("RGBA", (wsz, max(w.height for w in wheels)))
        x = 0
        for w in wheels:
            sheet.paste(w, (x, 0))
            x += w.width
        save(sheet, "widgets/wheels.png")
    if only_wheels or only:
        return
    # Standings with the thumbs shown after the flag.
    out = tmp / "rate.png"
    run(config({"standings": (50, 50)}, "dark", scale=2.0), out, size=(W // 2, H // 2), env={"TENTHS_EGUI_VOTING": "1"})
    alpha = np.array(Image.open(out).convert("RGBA"))[..., 3]
    box = bbox(alpha, (0, 0, W // 2, H // 2))
    if box:
        l, t, r, b = box
        save(composite(out, (max(l - pad, 0), max(t - pad, 0), r + pad, b + pad)), "widgets/standings-rate.png")


def settings(tmp):
    """The Settings window, drawn inside the screenshot (`--settings --shot`)."""
    print("settings")
    data = tmp / "settings-data"
    if data.exists():
        shutil.rmtree(data)
    (data / "reports").mkdir(parents=True)
    # A real race report from the demo race (crates/telemetry/examples/report_demo.rs).
    subprocess.run(["cargo", "run", "-q", "--profile", "fast", "-p", "telemetry", "--example", "report_demo", "--", str(data / "reports"), "14"],
                   cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = next(p.stem for p in (data / "reports").glob("*.json"))
    panes = [("general", "general.png", []), ("layouts", "layouts.png", []), ("appearance", "appearance.png", []),
             ("controls", "controls.png", []), ("pitstop", "pitstop.png", []), ("races", "races-list.png", []),
             ("races", "races.png", ["--report", report])]
    for pane, name, more in panes:
        out = tmp / f"settings-{name}"
        run(config(SHOWCASE, "dark"), out, size=(1600, 1200), args=["--settings", "--pane", pane, *more],
            env={"DATA_DIR": str(data)}, warmup=0, shot_after=6)
        alpha = np.array(Image.open(out).convert("RGBA"))[..., 3]
        box = bbox(alpha, (0, 0, 1600, 1200))
        im = Image.open(out).convert("RGBA").crop(box)
        bg = Image.new("RGBA", im.size, (24, 28, 34, 255))
        bg.alpha_composite(im)
        save(bg, f"settings/{name}")


def main():
    if not BIN.exists():
        sys.exit(f"Build the app first (cargo build --profile fast): {BIN} is missing")
    which = sys.argv[1:] or ["overview", "widgets", "settings"]
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        for step in which:
            {"overview": overview, "widgets": widgets, "wheels": wheels, "settings": settings}[step](tmp)


if __name__ == "__main__":
    main()
