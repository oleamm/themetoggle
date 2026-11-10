#!/usr/bin/env python3
"""Generate the ThemeToggle icon set from scratch.

Outputs (all written to assets/):
  tray-light.png   32x32 sun glyph with dark ink  -> shown when the taskbar is light
  tray-dark.png    32x32 moon glyph with light ink -> shown when the taskbar is dark
  themetoggle.ico  multi-size application icon embedded into the .exe

No third-party artwork is used: every pixel is rendered analytically from
implicit shapes with 4x4 supersampling, so the result is reproducible and the
script needs no imaging library. Re-run it after changing a shape or a colour.
"""

import math
import os
import struct
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "assets")
SS = 4  # supersamples per axis
TRAY_SIZE = 32
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def png_bytes(width, height, rgba):
    raw = b"".join(
        b"\x00" + bytes(rgba[y * width * 4:(y + 1) * width * 4]) for y in range(height)
    )

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def dist(px, py, cx, cy):
    return math.hypot(px - cx, py - cy)


def scaled(scale, inside):
    """Zoom a normalised shape about the icon centre."""

    def wrapper(x, y):
        return inside((x - 0.5) / scale + 0.5, (y - 0.5) / scale + 0.5)

    return wrapper


def sun_shape(x, y):
    cx = cy = 0.5
    if dist(x, y, cx, cy) <= 0.205:
        return True
    for index in range(8):
        angle = index * math.pi / 4.0
        ux, uy = math.cos(angle), math.sin(angle)
        # Capsule: a ray from the inner radius to the outer radius.
        t = min(max((x - cx) * ux + (y - cy) * uy, 0.305), 0.445)
        if dist(x, y, cx + ux * t, cy + uy * t) <= 0.047:
            return True
    return False


def moon_shape(x, y):
    if dist(x, y, 0.50, 0.52) > 0.40:
        return False
    return dist(x, y, 0.72, 0.40) > 0.34


sun_inside = scaled(0.84, sun_shape)
moon_inside = scaled(0.84, moon_shape)


def render_glyph(size, inside, ink):
    pixels = bytearray(size * size * 4)
    step = 1.0 / (size * SS)
    for py in range(size):
        for px in range(size):
            hits = 0
            for sy in range(SS):
                for sx in range(SS):
                    if inside((px * SS + sx + 0.5) * step, (py * SS + sy + 0.5) * step):
                        hits += 1
            offset = (py * size + px) * 4
            pixels[offset] = ink[0]
            pixels[offset + 1] = ink[1]
            pixels[offset + 2] = ink[2]
            pixels[offset + 3] = int(round(255 * hits / (SS * SS)))
    return pixels


BACKGROUND = (16, 24, 40)
DISC_LIGHT = (255, 255, 255)
DISC_DARK = (47, 111, 235)


def rounded_square(x, y, radius):
    if x < 0.0 or x > 1.0 or y < 0.0 or y > 1.0:
        return False
    dx = min(x, 1.0 - x)
    dy = min(y, 1.0 - y)
    if dx >= radius or dy >= radius:
        return True
    return dist(dx, dy, radius, radius) <= radius


def app_icon_color(x, y):
    """Rounded square with a half-filled contrast disc (the light/dark mark)."""
    if not rounded_square(x, y, 0.22):
        return None
    if dist(x, y, 0.5, 0.5) <= 0.30:
        return DISC_LIGHT if x < 0.5 else DISC_DARK
    return BACKGROUND


def render_app_icon(size):
    pixels = bytearray(size * size * 4)
    step = 1.0 / (size * SS)
    samples = SS * SS
    for py in range(size):
        for px in range(size):
            red = green = blue = covered = 0
            for sy in range(SS):
                for sx in range(SS):
                    color = app_icon_color(
                        (px * SS + sx + 0.5) * step, (py * SS + sy + 0.5) * step
                    )
                    if color is not None:
                        covered += 1
                        red += color[0]
                        green += color[1]
                        blue += color[2]
            offset = (py * size + px) * 4
            if covered:
                pixels[offset] = red // covered
                pixels[offset + 1] = green // covered
                pixels[offset + 2] = blue // covered
            pixels[offset + 3] = int(round(255 * covered / samples))
    return pixels


def dib_entry(size, rgba):
    """32bpp bottom-up DIB plus an empty AND mask, as used inside .ico files."""
    header = struct.pack(
        "<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, size * size * 4, 0, 0, 0, 0
    )
    body = bytearray()
    for y in range(size - 1, -1, -1):
        for x in range(size):
            offset = (y * size + x) * 4
            body += bytes(
                (rgba[offset + 2], rgba[offset + 1], rgba[offset], rgba[offset + 3])
            )
    mask = bytes((((size + 31) // 32) * 4) * size)
    return header + bytes(body) + mask


def ico_bytes(images):
    payloads = [
        png_bytes(size, size, rgba) if size >= 128 else dib_entry(size, rgba)
        for size, rgba in images
    ]
    offset = 6 + 16 * len(images)
    entries = []
    for (size, _), payload in zip(images, payloads):
        dimension = 0 if size >= 256 else size
        entries.append(
            struct.pack(
                "<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(payload), offset
            )
        )
        offset += len(payload)
    return (
        struct.pack("<HHH", 0, 1, len(images))
        + b"".join(entries)
        + b"".join(payloads)
    )


def preview_alpha(size, rgba):
    ramp = " .:-=+*#%@"
    for y in range(size):
        print(
            "".join(
                ramp[min(len(ramp) - 1, rgba[(y * size + x) * 4 + 3] * len(ramp) // 256)]
                for x in range(size)
            )
        )


def preview_color(size, rgba):
    ramp = " .:-=+*#%@"
    for y in range(size):
        line = ""
        for x in range(size):
            offset = (y * size + x) * 4
            if rgba[offset + 3] < 40:
                line += " "
                continue
            luminance = (rgba[offset] * 30 + rgba[offset + 1] * 59 + rgba[offset + 2] * 11) // 100
            line += ramp[min(len(ramp) - 1, luminance * len(ramp) // 256)]
        print(line)


def main():
    os.makedirs(ASSETS, exist_ok=True)

    sun = render_glyph(TRAY_SIZE, sun_inside, (32, 32, 32))
    moon = render_glyph(TRAY_SIZE, moon_inside, (245, 245, 245))
    for name, pixels in (("tray-light.png", sun), ("tray-dark.png", moon)):
        with open(os.path.join(ASSETS, name), "wb") as handle:
            handle.write(png_bytes(TRAY_SIZE, TRAY_SIZE, pixels))

    images = [(size, render_app_icon(size)) for size in ICO_SIZES]
    with open(os.path.join(ASSETS, "themetoggle.ico"), "wb") as handle:
        handle.write(ico_bytes(images))

    print("sun glyph (tray-light.png), 32x32:")
    preview_alpha(TRAY_SIZE, sun)
    print("\nmoon glyph (tray-dark.png), 32x32:")
    preview_alpha(TRAY_SIZE, moon)
    print("\napplication icon, 32x32 (brightness):")
    preview_color(32, dict(images)[32])
    print()
    for name in ("tray-light.png", "tray-dark.png", "themetoggle.ico"):
        print(f"{name}: {os.path.getsize(os.path.join(ASSETS, name))} bytes")


if __name__ == "__main__":
    main()
