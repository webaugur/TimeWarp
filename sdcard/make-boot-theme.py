#!/usr/bin/env python3
"""Wizard Plymouth theme for the grandfather clock.

The scene is the IndianaDell wizard (wand, sparkling Cherokee ᏃᏫᏍ) on black.
The Dell BGRT / OEM logo is not drawn. David's wizard PNGs are not stored in
this repository: IndianaDell has no LICENSE. Pass ``--wizard-src`` or let the
script fetch ``Themes/boot/overlay/wizard.png`` from the public GitHub repo.

Frames are composed for the framebuffer the splash will actually see:

* ``--kiosk kms`` (default): the kernel ``video=...,rotate=`` setting has
  already turned the console (``rotate=270`` for a clockwise ``--rotation``
  of 90; the kernel counts counter-clockwise), so frames are upright. Rotation 90 or 270 is a
  1024x1280 portrait canvas. Rotation 0 or 180 is 1280x1024.
* ``--kiosk labwc``: Plymouth runs before ``wlr-randr``, on the panel's native
  1280x1024 scanout. The upright scene is pre-rotated clockwise by
  ``--rotation`` so it looks upright on a monitor that is physically rotated.

A two-step Plymouth theme centers that animation. ``preview.gif`` is the
upright scene (portrait at the default rotation of 90), scaled down for review.
"""

from __future__ import annotations

import argparse
import math
import subprocess
import sys
import urllib.error
import urllib.request
from collections import deque
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from boot_config import output_size, pre_rotate_cw, viewer_size

FRAME_COUNT = 60
MAGIC_TEXT = "ᏃᏫᏍ"
THEME_NAME = "clockface"
WIZARD_FILES = ("wizard.png", "wizard-watermark.png")
RAW_BASE = (
    "https://raw.githubusercontent.com/webaugur/IndianaDell/main/Themes/boot/overlay"
)
REPO_URL = "https://github.com/webaugur/IndianaDell.git"
FONT_URL = (
    "https://github.com/notofonts/cherokee/raw/main/fonts/"
    "NotoSansCherokee/full/ttf/NotoSansCherokee-Regular.ttf"
)
FONT_CANDIDATES = (
    "/usr/share/fonts/truetype/noto/NotoSansCherokee-Regular.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCherokee-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansCherokee-Bold.ttf",
)


def rotate_cw(image: Image.Image, degrees: int) -> Image.Image:
    degrees %= 360
    if degrees == 0:
        return image
    if degrees == 90:
        return image.transpose(Image.Transpose.ROTATE_270)
    if degrees == 180:
        return image.transpose(Image.Transpose.ROTATE_180)
    if degrees == 270:
        return image.transpose(Image.Transpose.ROTATE_90)
    raise ValueError(f"rotation must be a multiple of 90 (got {degrees})")


def _download(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    print(f"fetch {url}")
    urllib.request.urlretrieve(url, dest)


def fetch_wizard(cache: Path) -> Path:
    """Return a local wizard PNG, downloading it if the cache is empty.

    IndianaDell was public on 2026-11-15 (GitHub ``private: false``) and has
    no LICENSE file. A private or moved repo fails this fetch; point
    ``--wizard-src`` at a local checkout of ``Themes/boot/overlay`` instead.
    """
    cache.mkdir(parents=True, exist_ok=True)
    missing = [name for name in WIZARD_FILES if not (cache / name).is_file()]
    if missing:
        try:
            for name in missing:
                _download(f"{RAW_BASE}/{name}", cache / name)
        except (OSError, urllib.error.URLError) as err:
            print(f"raw download failed ({err}); trying a sparse clone", file=sys.stderr)
            _sparse_clone(cache)
    chosen = _prefer_wizard(cache)
    if chosen is None:
        raise SystemExit(
            "wizard art not found. IndianaDell is fetched at build time and is "
            "not committed here. Pass --wizard-src DIR (a checkout or a folder "
            f"containing {', '.join(WIZARD_FILES)}), or clone {REPO_URL}"
        )
    return chosen


def _sparse_clone(cache: Path) -> None:
    dest = cache / "IndianaDell"
    if not (dest / ".git").is_dir():
        subprocess.run(
            [
                "git",
                "clone",
                "--depth",
                "1",
                "--filter=blob:none",
                "--sparse",
                REPO_URL,
                str(dest),
            ],
            check=True,
        )
        subprocess.run(
            ["git", "sparse-checkout", "set", "Themes/boot/overlay"],
            cwd=dest,
            check=True,
        )
    overlay = dest / "Themes" / "boot" / "overlay"
    for name in WIZARD_FILES:
        src = overlay / name
        if src.is_file():
            (cache / name).write_bytes(src.read_bytes())


def _prefer_wizard(directory: Path) -> Path | None:
    """Prefer the RGBA sprite. The watermark is the fallback (white knocked out)."""
    primary = directory / "wizard.png"
    if primary.is_file():
        return primary
    watermark = directory / "wizard-watermark.png"
    if watermark.is_file():
        return watermark
    return None


def resolve_wizard(wizard_src: Path | None, cache: Path, synthetic: bool) -> Path | None:
    if synthetic:
        return None
    if wizard_src is not None:
        if wizard_src.is_file():
            return wizard_src
        if wizard_src.is_dir():
            found = _prefer_wizard(wizard_src)
            if found is None:
                found = _prefer_wizard(wizard_src / "Themes" / "boot" / "overlay")
            if found is None:
                raise SystemExit(f"no wizard PNG under {wizard_src}")
            return found
        raise SystemExit(f"--wizard-src not found: {wizard_src}")
    return fetch_wizard(cache)


def knock_out_background(image: Image.Image) -> Image.Image:
    """Make a flat white or paper background transparent. RGBA art is kept."""
    image = image.convert("RGBA")
    raw = list(image.getdata())
    alpha_mean = sum(px[3] for px in raw) / max(1, len(raw))
    if alpha_mean < 250:
        return _crop_alpha(image)
    out = []
    for red, green, blue, _alpha in raw:
        dist = math.sqrt((red - 255) ** 2 + (green - 255) ** 2 + (blue - 255) ** 2)
        alpha = int(max(0.0, min(1.0, (dist - 12.0) / 30.0)) * 255)
        out.append((red, green, blue, alpha))
    image.putdata(out)
    return _crop_alpha(image)


# Eye whites in wizard.png are transparent, so a black splash shows through
# them and the dark pupils read as empty sockets. Other interior gaps (the
# scroll, hair, sparkle cutouts) stay clear.
_SCLERA = (255, 250, 244, 255)
_CLEAR_ALPHA = 48


def _luminance(red: int, green: int, blue: int) -> float:
    return 0.299 * red + 0.587 * green + 0.114 * blue


def _enclosed_clear(image: Image.Image) -> list[list[tuple[int, int]]]:
    """Transparent components that do not touch the image border."""
    width, height = image.size
    pixels = image.load()
    outside = [[False] * width for _ in range(height)]
    queue: deque[tuple[int, int]] = deque()

    def clear(x: int, y: int) -> bool:
        return pixels[x, y][3] < _CLEAR_ALPHA

    for x in range(width):
        for y in (0, height - 1):
            if clear(x, y) and not outside[y][x]:
                outside[y][x] = True
                queue.append((x, y))
    for y in range(height):
        for x in (0, width - 1):
            if clear(x, y) and not outside[y][x]:
                outside[y][x] = True
                queue.append((x, y))
    while queue:
        x, y = queue.popleft()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < width and 0 <= ny < height and not outside[ny][nx] and clear(nx, ny):
                outside[ny][nx] = True
                queue.append((nx, ny))

    visited = [[False] * width for _ in range(height)]
    components: list[list[tuple[int, int]]] = []
    for y in range(height):
        for x in range(width):
            if not (clear(x, y) and not outside[y][x] and not visited[y][x]):
                continue
            queue = deque([(x, y)])
            visited[y][x] = True
            cells: list[tuple[int, int]] = []
            while queue:
                cx, cy = queue.popleft()
                cells.append((cx, cy))
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    nx, ny = cx + dx, cy + dy
                    if (
                        0 <= nx < width
                        and 0 <= ny < height
                        and clear(nx, ny)
                        and not outside[ny][nx]
                        and not visited[ny][nx]
                    ):
                        visited[ny][nx] = True
                        queue.append((nx, ny))
            components.append(cells)
    return components


def _has_filled_pupil(image: Image.Image, cells: list[tuple[int, int]]) -> bool:
    """True when a compact dark blob sits inside this hole.

    wizard.png anti-aliases the pupil, so the transparent sclera does not
    share an edge with the darkest pixels. The blob is measured inside the
    hole's box instead. A thin outline or a robe shadow is not dense enough.
    """
    pixels = image.load()
    xs = [x for x, _y in cells]
    ys = [y for _x, y in cells]
    dark: list[tuple[int, int]] = []
    for y in range(min(ys), max(ys) + 1):
        for x in range(min(xs), max(xs) + 1):
            red, green, blue, alpha = pixels[x, y]
            if alpha >= 200 and _luminance(red, green, blue) < 70:
                dark.append((x, y))
    if len(dark) < 8:
        return False
    dxs = [x for x, _y in dark]
    dys = [y for _x, y in dark]
    box = (max(dxs) - min(dxs) + 1) * (max(dys) - min(dys) + 1)
    return len(dark) / box >= 0.55


def _eye_pair(image: Image.Image, components: list[list[tuple[int, int]]]) -> list[list[tuple[int, int]]]:
    """The two face sockets: similar ovals, side by side, each holding a pupil."""
    candidates: list[tuple[float, float, float, float, list[tuple[int, int]]]] = []
    for cells in components:
        if len(cells) < 24:
            continue
        xs = [x for x, _y in cells]
        ys = [y for _x, y in cells]
        box_w = max(xs) - min(xs) + 1
        box_h = max(ys) - min(ys) + 1
        if box_w < 6 or box_h < 8:
            continue
        aspect = box_w / box_h
        if not 0.35 <= aspect <= 1.6:
            continue
        if len(cells) < 0.25 * box_w * box_h:
            continue
        if not _has_filled_pupil(image, cells):
            continue
        cx = sum(xs) / len(xs)
        cy = sum(ys) / len(ys)
        candidates.append((cx, cy, float(box_w), float(len(cells)), cells))
    best: list[list[tuple[int, int]]] | None = None
    best_score = -1.0
    for index, left in enumerate(candidates):
        for right in candidates[index + 1 :]:
            if abs(left[1] - right[1]) > max(left[2], right[2]) * 0.45:
                continue
            gap = abs(left[0] - right[0])
            span = (left[2] + right[2]) / 2
            if not span * 0.6 <= gap <= span * 4.0:
                continue
            area_ratio = max(left[3], right[3]) / max(1.0, min(left[3], right[3]))
            if area_ratio > 2.5:
                continue
            score = left[3] + right[3]
            if score > best_score:
                best_score = score
                best = [left[4], right[4]]
    return best or []


def fill_hollow_eyes(image: Image.Image) -> Image.Image:
    """Paint opaque sclera into the wizard's transparent eye whites.

    Pupils are already dark opaque pixels inside those sockets and are left
    as they are. A second hole in the silhouette (scroll, sparkle, hair) is
    not a paired socket touching a pupil, so it stays transparent.
    """
    image = image.convert("RGBA")
    sockets = _eye_pair(image, _enclosed_clear(image))
    if not sockets:
        return image
    pixels = image.load()
    width, height = image.size
    for cells in sockets:
        xs = [x for x, _y in cells]
        ys = [y for _x, y in cells]
        x0 = max(0, min(xs) - 2)
        y0 = max(0, min(ys) - 2)
        x1 = min(width - 1, max(xs) + 2)
        y1 = min(height - 1, max(ys) + 2)
        # Stay inside the socket. Flooding the whole sprite would leak through
        # any soft edge into the transparent background.
        seen: set[tuple[int, int]] = set()
        queue: deque[tuple[int, int]] = deque(cells)
        for cell in cells:
            seen.add(cell)
        while queue:
            x, y = queue.popleft()
            if pixels[x, y][3] < 200:
                pixels[x, y] = _SCLERA
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                nx, ny = x + dx, y + dy
                if not (x0 <= nx <= x1 and y0 <= ny <= y1) or (nx, ny) in seen:
                    continue
                if pixels[nx, ny][3] >= 200:
                    continue
                seen.add((nx, ny))
                queue.append((nx, ny))
    return image


def _crop_alpha(image: Image.Image, threshold: int = 16, pad: int = 2) -> Image.Image:
    alpha = image.getchannel("A")
    bbox = alpha.point(lambda value: 255 if value > threshold else 0).getbbox()
    if bbox is None:
        return image
    left = max(0, bbox[0] - pad)
    top = max(0, bbox[1] - pad)
    right = min(image.width, bbox[2] + pad)
    bottom = min(image.height, bbox[3] + pad)
    return image.crop((left, top, right, bottom))


def wand_tip(sprite: Image.Image) -> tuple[float, float]:
    """Brightest pink or white spark in the upper half of the sprite."""
    raw = sprite.load()
    best = -1.0
    tip = (sprite.width * 0.72, sprite.height * 0.18)
    limit_y = int(sprite.height * 0.55)
    for y in range(limit_y):
        for x in range(sprite.width):
            red, green, blue, alpha = raw[x, y]
            if alpha < 40:
                continue
            lum = 0.299 * red + 0.587 * green + 0.114 * blue
            pink = (red + blue) - 1.2 * green
            score = lum + 0.8 * max(0.0, pink)
            if lum > 220:
                score += 80.0
            if score > best:
                best = score
                tip = (float(x), float(y))
    return tip


def synthetic_wizard() -> tuple[Image.Image, tuple[float, float]]:
    """A stand-in hat and wand so tests can run without David's PNG."""
    image = Image.new("RGBA", (160, 200), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    white = (245, 248, 255, 235)
    draw.polygon([(80, 78), (38, 175), (122, 175)], fill=white)
    draw.polygon([(52, 62), (80, 8), (108, 62)], fill=(255, 255, 255, 255))
    draw.ellipse([62, 48, 98, 88], fill=(230, 235, 255, 230))
    draw.line([(108, 96), (150, 36)], fill=(220, 200, 140, 255), width=3)
    draw.ellipse([144, 28, 158, 42], fill=(255, 250, 200, 255))
    return image, (151.0, 34.0)


def find_font(cache: Path, explicit: Path | None, size: int) -> ImageFont.ImageFont:
    if explicit is not None:
        return ImageFont.truetype(str(explicit), size)
    for path in FONT_CANDIDATES:
        if Path(path).is_file():
            return ImageFont.truetype(path, size)
    dest = cache / "NotoSansCherokee-Regular.ttf"
    if not dest.is_file():
        _download(FONT_URL, dest)
    return ImageFont.truetype(str(dest), size)


def render_magic_text(font: ImageFont.ImageFont) -> tuple[Image.Image, tuple[int, int, int, int]]:
    probe = ImageDraw.Draw(Image.new("RGBA", (8, 8)))
    bbox = probe.textbbox((0, 0), MAGIC_TEXT, font=font)
    width = bbox[2] - bbox[0]
    height = bbox[3] - bbox[1]
    pad = 28
    image = Image.new("RGBA", (width + pad * 2, height + pad * 2), (0, 0, 0, 0))
    origin = (pad - bbox[0], pad - bbox[1])
    glow = Image.new("RGBA", image.size, (0, 0, 0, 0))
    ImageDraw.Draw(glow).text(origin, MAGIC_TEXT, font=font, fill=(200, 220, 255, 180))
    glow = glow.filter(ImageFilter.GaussianBlur(6))
    image = Image.alpha_composite(image, glow)
    ImageDraw.Draw(image).text(origin, MAGIC_TEXT, font=font, fill=(255, 255, 255, 255))
    return image, (pad, pad, pad + width, pad + height)


def _star(draw: ImageDraw.ImageDraw, x: float, y: float, size: float, color: tuple[int, int, int, int]) -> None:
    arm = max(1, int(size * 0.7))
    draw.line([(x - size * 2.2, y), (x + size * 2.2, y)], fill=color, width=arm)
    draw.line([(x, y - size * 2.2), (x, y + size * 2.2)], fill=color, width=arm)
    draw.ellipse([x - size * 0.4, y - size * 0.4, x + size * 0.4, y + size * 0.4], fill=color)


def sparkle_layer(
    width: int,
    height: int,
    text_box: tuple[int, int, int, int],
    frame_index: int,
    frame_count: int,
    wand: tuple[float, float],
) -> Image.Image:
    layer = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    x0, y0, x1, y1 = text_box
    cx = (x0 + x1) / 2
    cy = (y0 + y1) / 2
    tw = max(1, x1 - x0)
    th = max(1, y1 - y0)
    t = frame_index / frame_count
    anchors: list[tuple[float, float, float, float]] = []
    for i in range(28):
        angle = 2 * math.pi * i / 28 + 0.4 * math.sin(i * 1.7)
        px = cx + (tw * 0.68 + 18 * math.sin(i * 2.3)) * math.cos(angle)
        py = cy + (th * 1.05 + 14 * math.cos(i * 1.9)) * math.sin(angle) * 0.78
        anchors.append((px, py, 2.0 + (i % 4) * 1.0, (i * 0.37) % 1.0))
    for px, py, size, phase in anchors:
        twinkle = 0.5 + 0.5 * math.sin(2 * math.pi * (t * 2.5 + phase))
        dist = math.hypot(px - wand[0], py - wand[1])
        boost = max(0.0, 1.0 - dist / 180.0) ** 1.3
        alpha = int(255 * min(1.0, 0.35 + 0.55 * twinkle + 0.65 * boost))
        if alpha < 18:
            continue
        scale = size * (0.75 + 0.65 * twinkle + 0.4 * boost)
        _star(draw, px, py, scale, (255, 250, 220, alpha))
    for i in range(18):
        u = (t * 1.6 + i / 18) % 1.0
        mx = wand[0] * (1 - u) + cx * u + 4.0 * math.sin(i * 2.7 + t * 8)
        my = wand[1] * (1 - u) + cy * u - 24 * math.sin(math.pi * u)
        pulse = 0.55 + 0.45 * math.sin(2 * math.pi * (t * 4 + i * 0.17))
        alpha = int(220 * pulse)
        if alpha < 28:
            continue
        radius = 1.4 + 2.0 * pulse
        draw.ellipse(
            [mx - radius, my - radius, mx + radius, my + radius],
            fill=(255, int(200 + 40 * (1 - u)), int(220 + 25 * u), alpha),
        )
    bloom = layer.filter(ImageFilter.GaussianBlur(1.4))
    return Image.alpha_composite(bloom, layer)


def compose_upright(
    wizard: Image.Image,
    tip: tuple[float, float],
    text: Image.Image,
    ink: tuple[int, int, int, int],
    frame_index: int,
    frame_count: int,
    size: tuple[int, int],
) -> Image.Image:
    width, height = size
    canvas = Image.new("RGBA", size, (0, 0, 0, 255))
    target_h = max(1, int(height * 0.42))
    scale = target_h / wizard.height
    wizard_s = wizard.resize(
        (max(1, int(wizard.width * scale)), target_h),
        Image.Resampling.LANCZOS,
    )
    tip_s = (tip[0] * scale, tip[1] * scale)
    text_scale = max(0.6, min(1.6, height / 1280))
    if abs(text_scale - 1.0) > 0.05:
        text_s = text.resize(
            (max(1, int(text.width * text_scale)), max(1, int(text.height * text_scale))),
            Image.Resampling.LANCZOS,
        )
        ink_s = tuple(int(v * text_scale) for v in ink)
    else:
        text_s = text
        ink_s = ink
    gap = int(width * 0.02)
    group_w = wizard_s.width + gap + text_s.width
    group_x = max(0, (width - group_w) // 2)
    group_y = max(0, (height - max(wizard_s.height, text_s.height)) // 2)
    wiz_x = group_x
    wiz_y = group_y + max(0, (max(wizard_s.height, text_s.height) - wizard_s.height) // 2)
    text_x = group_x + wizard_s.width + gap
    text_y = group_y + int(wizard_s.height * 0.34)
    canvas.alpha_composite(wizard_s, (wiz_x, wiz_y))
    t = frame_index / frame_count
    shimmer = 0.90 + 0.10 * (0.5 + 0.5 * math.sin(2 * math.pi * (t * 2.0)))
    faded = text_s.copy()
    bands = faded.getchannel("A").point(lambda value: int(value * shimmer))
    faded.putalpha(bands)
    canvas.alpha_composite(faded, (text_x, text_y))
    ink_abs = (
        text_x + ink_s[0],
        text_y + ink_s[1],
        text_x + ink_s[2],
        text_y + ink_s[3],
    )
    sparks = sparkle_layer(
        width,
        height,
        ink_abs,
        frame_index,
        frame_count,
        (wiz_x + tip_s[0], wiz_y + tip_s[1]),
    )
    return Image.alpha_composite(canvas, sparks)


def write_plymouth(theme_dir: Path) -> None:
    image_dir = f"/usr/share/plymouth/themes/{THEME_NAME}"
    text = f"""[Plymouth Theme]
Name=Clockface
Description=Wizard boot splash for the grandfather clock (no OEM logo)
ModuleName=two-step

[two-step]
Font=Ubuntu 12
TitleFont=Ubuntu 12
ImageDir={image_dir}
DialogHorizontalAlignment=.5
DialogVerticalAlignment=.82
TitleHorizontalAlignment=.5
TitleVerticalAlignment=.5
HorizontalAlignment=.5
VerticalAlignment=.5
WatermarkHorizontalAlignment=.5
WatermarkVerticalAlignment=.96
Transition=none
TransitionDuration=0.0
BackgroundStartColor=0x000000
BackgroundEndColor=0x000000
ProgressBarBackgroundColor=0x000000
ProgressBarForegroundColor=0x000000
DialogClearsFirmwareBackground=true
MessageBelowAnimation=true

[boot-up]
UseEndAnimation=false
UseFirmwareBackground=false

[shutdown]
UseEndAnimation=false
UseFirmwareBackground=false

[reboot]
UseEndAnimation=false
UseFirmwareBackground=false

[updates]
SuppressMessages=true
ProgressBarShowPercentComplete=false
UseProgressBar=false

[system-upgrade]
SuppressMessages=true
UseProgressBar=false
"""
    (theme_dir / f"{THEME_NAME}.plymouth").write_text(text, encoding="utf-8")
    Image.new("RGB", (32, 32), (0, 0, 0)).save(theme_dir / "background.png")


def write_preview(frames: list[Image.Image], path: Path, height: int = 640) -> None:
    scaled: list[Image.Image] = []
    for frame in frames:
        ratio = height / frame.height
        size = (max(1, int(frame.width * ratio)), height)
        reduced = frame.resize(size, Image.Resampling.LANCZOS).convert(
            "P", palette=Image.Palette.ADAPTIVE, colors=64
        )
        scaled.append(reduced)
    path.parent.mkdir(parents=True, exist_ok=True)
    scaled[0].save(
        path,
        save_all=True,
        append_images=scaled[1:],
        duration=50,
        loop=0,
        optimize=True,
    )


def build_theme(
    *,
    out_dir: Path,
    rotation: int,
    kiosk: str,
    frames: int,
    wizard_path: Path | None,
    font: ImageFont.ImageFont | None,
    preview: Path | None,
) -> None:
    view = viewer_size(rotation)
    panel = output_size(rotation, kiosk)
    turn = pre_rotate_cw(rotation, kiosk)
    if wizard_path is None:
        sprite, tip = synthetic_wizard()
    else:
        sprite = fill_hollow_eyes(knock_out_background(Image.open(wizard_path)))
        tip = wand_tip(sprite)
        print(f"wizard {wizard_path} {sprite.size} tip=({tip[0]:.0f},{tip[1]:.0f})")
    if font is None:
        text = Image.new("RGBA", (8, 8), (0, 0, 0, 0))
        ink = (0, 0, 8, 8)
    else:
        text, ink = render_magic_text(font)
        if text.getchannel("A").getbbox() is None:
            print("warning: Cherokee text drew nothing; check the font", file=sys.stderr)
    out_dir.mkdir(parents=True, exist_ok=True)
    for stale in out_dir.glob("animation-*.png"):
        stale.unlink()
    for stale in out_dir.glob("throbber-*.png"):
        stale.unlink()
    upright_frames: list[Image.Image] = []
    for index in range(frames):
        upright = compose_upright(sprite, tip, text, ink, index, frames, view)
        upright_frames.append(upright)
        panel_frame = rotate_cw(upright, turn)
        if panel_frame.size != panel:
            raise RuntimeError(f"frame size {panel_frame.size} != {panel}")
        name = f"{index + 1:04d}"
        panel_frame.save(out_dir / f"animation-{name}.png", optimize=True)
        panel_frame.save(out_dir / f"throbber-{name}.png", optimize=True)
        if (index + 1) % 10 == 0 or index == 0:
            print(f"  frame {index + 1:02d}/{frames} upright={view} panel={panel} rotate={turn}")
    write_plymouth(out_dir)
    (out_dir / "rotation.txt").write_text(
        f"rotation={rotation}\nkiosk={kiosk}\nupright={view[0]}x{view[1]}\n"
        f"panel={panel[0]}x{panel[1]}\npre_rotate_cw={turn}\nframes={frames}\n",
        encoding="utf-8",
    )
    gif = preview if preview is not None else out_dir / "preview.gif"
    write_preview(upright_frames, gif)
    print(f"theme {out_dir}")
    print(f"preview {gif}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True, help="theme directory to write")
    parser.add_argument("--rotation", type=int, default=90)
    parser.add_argument("--kiosk", choices=("kms", "labwc"), default="kms")
    parser.add_argument("--frames", type=int, default=FRAME_COUNT)
    parser.add_argument(
        "--wizard-src",
        type=Path,
        default=None,
        help="local wizard PNG, overlay directory, or IndianaDell checkout",
    )
    parser.add_argument("--cache", type=Path, default=None, help="download cache (default: next to --out)")
    parser.add_argument("--font", type=Path, default=None, help="Cherokee-capable TTF")
    parser.add_argument("--preview", type=Path, default=None, help="where to write the upright GIF")
    parser.add_argument(
        "--synthetic",
        action="store_true",
        help="draw a stand-in wizard and skip the IndianaDell download (tests)",
    )
    parser.add_argument("--no-text", action="store_true", help="skip the Cherokee line")
    args = parser.parse_args(argv)
    if args.frames < 1:
        print("need at least one frame", file=sys.stderr)
        return 2
    cache = args.cache if args.cache is not None else args.out.parent / "cache"
    wizard = resolve_wizard(args.wizard_src, cache / "wizard", args.synthetic)
    font = None
    if not args.no_text and not args.synthetic:
        font = find_font(cache / "fonts", args.font, 58)
    elif not args.no_text and args.synthetic and args.font is not None:
        font = ImageFont.truetype(str(args.font), 58)
    build_theme(
        out_dir=args.out,
        rotation=args.rotation,
        kiosk=args.kiosk,
        frames=args.frames,
        wizard_path=wizard,
        font=font,
        preview=args.preview,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
