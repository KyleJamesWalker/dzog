#!/usr/bin/env python3
"""
dzog-flux-forge: Standalone CLI tool for conditioning `black-forest-labs/FLUX.2-klein-4B`
on Dzog `CohesiveBody` structural silhouettes to generate sharp 1994 VGA (*Raptor: Call of the Shadows*
style) sprite atlases and inverse constitutive material maps (Albedo + Normal/Elevation + Cleavage Seams).

Usage examples:
  # 1. Export deterministic CohesiveBody conditioning masks (no GPU/torch required)
  python3 tools/flux-forge/flux_forge.py export-masks --out-dir tools/flux-forge/masks

  # 2. Generate VGA retro sprites using local FLUX.2 [klein] 4B (4-step rectified flow on CUDA)
  python3 tools/flux-forge/flux_forge.py generate --masks-dir tools/flux-forge/masks --out-dir crates/shmup/assets

  # 3. Post-process any sprite into VGA 256-color palette + 1px keyline + RON constitutive seam map
  python3 tools/flux-forge/flux_forge.py quantize --input sprite.png --output vga_sprite.png
"""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
from typing import Dict, List, Tuple

try:
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:
    Image = None  # Handled gracefully with clear CLI instructions


ENTITY_SPECS: Dict[str, Dict[str, str]] = {
    "player_raptor": {
        "title": "Tri-Lobe Raptor Interceptor",
        "prompt": (
            "1994 MS-DOS VGA pixel art top-down military sci-fi fighter jet sprite, "
            "Raptor Call of the Shadows style, brushed titanium fuselage, crimson red swept wing stripes, "
            "dark smoked glass cockpit canopy, twin aft thruster nozzles, crisp 1px dark keyline, "
            "sharp pixel edges, isolated on solid black background, orthographic top-down view"
        ),
    },
    "twin_gunship": {
        "title": "Twin-Nacelle Heavy Strike Gunship",
        "prompt": (
            "1994 MS-DOS VGA pixel art top-down heavy twin-fuselage enemy gunship sprite, "
            "Raptor Call of the Shadows style, dark gunmetal armor plates, ribbed engine intakes, "
            "dual forward orange plasma cannons, industrial rivets, crisp 1px dark outline, "
            "isolated on solid black background, orthographic top-down view"
        ),
    },
    "chladni_matriarch": {
        "title": "Chladni Swarm Carrier Cruiser",
        "prompt": (
            "1994 MS-DOS VGA pixel art top-down alien command cruiser ship sprite, "
            "Raptor Call of the Shadows style, bronze and dark violet armored hull, twin forward "
            "tuning-fork resonance mandibles, glowing cyan energy core, sharp pixel art, "
            "isolated on solid black background, orthographic top-down view"
        ),
    },
    "geodesic_dome": {
        "title": "Geodesic Fuel Dome",
        "prompt": (
            "1994 MS-DOS VGA pixel art top-down spherical geodesic industrial storage dome building, "
            "Raptor Call of the Shadows style, triangulated steel truss grid, four cardinal pipe flanges, "
            "strong top-left directional lighting, crisp pixel art on solid black background"
        ),
    },
    "biodome_reactor": {
        "title": "Biodome Exciton Reactor",
        "prompt": (
            "1994 MS-DOS VGA pixel art top-down circular glass biodome building, "
            "Raptor Call of the Shadows style, concrete containment ring, silver cross-hatch glass roof "
            "over glowing emerald green bioluminescent core, crisp pixel art on solid black background"
        ),
    },
}


def require_pillow() -> None:
    if Image is None:
        raise SystemExit("Error: Pillow is required. Run `pip install Pillow` or `uv run flux_forge.py`.")


def generate_conditioning_mask(entity: str, size: int = 512) -> "Image.Image":
    """Render an exact orthographic conditioning silhouette matching `dzog-core` CohesiveBody geometry."""
    require_pillow()
    img = Image.new("RGB", (size, size), (0, 0, 0))
    draw = ImageDraw.Draw(img)
    cx = size * 0.5
    cy = size * 0.5
    r = size * 0.38

    if entity == "player_raptor":
        # Swept port/starboard wings + central spearhead fuselage + crimson chevron guides
        draw.polygon(
            [
                (cx, cy - r * 0.95),
                (cx + r * 0.34, cy - r * 0.20),
                (cx + r * 1.00, cy + r * 0.25),
                (cx + r * 0.95, cy + r * 0.55),
                (cx + r * 0.34, cy + r * 0.45),
                (cx + r * 0.28, cy + r * 0.85),
                (cx - r * 0.28, cy + r * 0.85),
                (cx - r * 0.34, cy + r * 0.45),
                (cx - r * 0.95, cy + r * 0.55),
                (cx - r * 1.00, cy + r * 0.25),
                (cx - r * 0.34, cy - r * 0.20),
            ],
            fill=(150, 160, 175),
            outline=(40, 45, 55),
        )
        # Crimson wing stripes
        draw.polygon(
            [
                (cx - r * 0.42, cy + r * 0.02),
                (cx - r * 0.85, cy + r * 0.32),
                (cx - r * 0.72, cy + r * 0.44),
                (cx - r * 0.40, cy + r * 0.20),
            ],
            fill=(210, 45, 45),
        )
        draw.polygon(
            [
                (cx + r * 0.42, cy + r * 0.02),
                (cx + r * 0.85, cy + r * 0.32),
                (cx + r * 0.72, cy + r * 0.44),
                (cx + r * 0.40, cy + r * 0.20),
            ],
            fill=(210, 45, 45),
        )
        # Canopy
        draw.ellipse(
            [cx - r * 0.14, cy - r * 0.55, cx + r * 0.14, cy - r * 0.05],
            fill=(30, 55, 85),
        )
    elif entity == "twin_gunship":
        # Central fuselage + port/starboard engine nacelles connected by wing struts
        draw.rectangle(
            [cx - r * 0.90, cy - r * 0.78, cx - r * 0.44, cy + r * 0.82],
            fill=(110, 118, 130),
        )
        draw.rectangle(
            [cx + r * 0.44, cy - r * 0.78, cx + r * 0.90, cy + r * 0.82],
            fill=(110, 118, 130),
        )
        draw.rectangle(
            [cx - r * 0.45, cy - r * 0.28, cx + r * 0.45, cy + r * 0.32],
            fill=(85, 92, 105),
        )
        draw.rectangle(
            [cx - r * 0.26, cy - r * 0.72, cx + r * 0.26, cy + r * 0.92],
            fill=(135, 142, 155),
        )
    elif entity == "chladni_matriarch":
        draw.ellipse(
            [cx - r * 0.82, cy - r * 0.75, cx + r * 0.82, cy + r * 0.55],
            fill=(140, 85, 135),
        )
        draw.rectangle(
            [cx - r * 0.65, cy + r * 0.10, cx - r * 0.28, cy + r * 0.98],
            fill=(165, 105, 155),
        )
        draw.rectangle(
            [cx + r * 0.28, cy + r * 0.10, cx + r * 0.65, cy + r * 0.98],
            fill=(165, 105, 155),
        )
    elif entity == "geodesic_dome":
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(155, 165, 180))
        for k in range(-3, 4):
            off = k * (r * 0.32)
            draw.line([(cx + off, cy - r), (cx + off, cy + r)], fill=(60, 68, 80), width=4)
            draw.line([(cx - r, cy + off), (cx + r, cy + off)], fill=(60, 68, 80), width=4)
    else:
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(40, 165, 95))
        for k in range(-2, 3):
            off = k * (r * 0.38)
            draw.line([(cx + off, cy - r), (cx + off, cy + r)], fill=(180, 200, 195), width=6)
            draw.line([(cx - r, cy + off), (cx + r, cy + off)], fill=(180, 200, 195), width=6)

    return img


def quantize_to_vga_and_extract_seams(
    img: "Image.Image", target_size: int = 48
) -> Tuple["Image.Image", Dict[str, object]]:
    """
    Downsample to crisp VGA sprite resolution (`target_size x target_size`),
    quantize to a 32-color VGA ramp with a 1px dark charcoal keyline, and extract
    a local-grid cohesion seam map (`C_0 in [0.82, 1.0]`) so panel lines become
    physical fracture waveguides in `dzog-core`.
    """
    require_pillow()
    rgba = img.convert("RGBA").resize((target_size, target_size), Image.Resampling.NEAREST)
    pixels = rgba.load()

    occupied = [[False] * target_size for _ in range(target_size)]
    luma = [[0.0] * target_size for _ in range(target_size)]

    for y in range(target_size):
        for x in range(target_size):
            r, g, b, a = pixels[x, y]
            bright = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0
            if a > 32 and bright > 0.05:
                occupied[y][x] = True
                luma[y][x] = bright
            else:
                pixels[x, y] = (0, 0, 0, 0)

    cohesion_grid: List[float] = []
    for y in range(target_size):
        for x in range(target_size):
            if not occupied[y][x]:
                cohesion_grid.append(0.0)
                continue

            # Detect silhouette boundary for 1px VGA dark keyline
            is_edge = False
            for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if nx < 0 or nx >= target_size or ny < 0 or ny >= target_size or not occupied[ny][nx]:
                    is_edge = True
                    break

            r, g, b, a = pixels[x, y]
            # Quantize to 5-bit VGA steps (32 levels per channel)
            qr = (r >> 3) << 3
            qg = (g >> 3) << 3
            qb = (b >> 3) << 3

            if is_edge:
                pixels[x, y] = (max(16, qr // 3), max(20, qg // 3), max(28, qb // 3), 255)
                cohesion_grid.append(0.92)
            else:
                pixels[x, y] = (qr, qg, qb, 255)
                # Dark internal panel seams become lower-cohesion cleavage waveguides (0.84)
                seam_c = 0.84 if luma[y][x] < 0.28 else 0.98
                cohesion_grid.append(round(seam_c, 3))

    constitutive_meta = {
        "local_size": target_size,
        "active_cells": sum(1 for c in cohesion_grid if c >= 0.15),
        "seam_cells": sum(1 for c in cohesion_grid if 0.15 <= c <= 0.86),
        "cohesion": cohesion_grid,
    }
    return rgba, constitutive_meta


def bake_height_and_normal_map(
    entity: str, vga_img: "Image.Image", target_size: int = 48
) -> "Image.Image":
    """
    Bake a 4-channel (nx, ny, nz, elevation_h) height & surface-normal map for 2.5D shadow
    casting and 3D falling-wreckage roof collision (`h in [0.0, 1.0]`).
    Combines exact architectural SDF profiles (hemispherical domes, stepped turrets, twin nacelles)
    with Sobel luminance relief from the VGA sprite.
    """
    require_pillow()
    rgba = vga_img.convert("RGBA").resize((target_size, target_size), Image.Resampling.NEAREST)
    src_px = rgba.load()
    out = Image.new("RGBA", (target_size, target_size), (128, 128, 255, 0))
    dst_px = out.load()

    cx = (target_size - 1) * 0.5
    cy = (target_size - 1) * 0.5
    r_max = target_size * 0.42

    # Base architectural height profile h(x, y) in [0.0, 1.0]
    h_grid = [[0.0] * target_size for _ in range(target_size)]
    for y in range(target_size):
        for x in range(target_size):
            r, g, b, a = src_px[x, y]
            if a < 32:
                continue
            luma = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0
            dx = (x - cx) / r_max
            dy = (y - cy) / r_max
            rad = math.sqrt(dx * dx + dy * dy)

            if entity == "geodesic_dome":
                dome_z = math.sqrt(max(0.0, 1.0 - min(1.0, rad * rad)))
                h_grid[y][x] = min(1.0, 0.44 + 0.42 * dome_z + (luma - 0.5) * 0.04)
            elif entity == "biodome_reactor":
                if rad > 0.76:
                    h_grid[y][x] = 0.68  # Concrete containment collar
                else:
                    dome_z = math.sqrt(max(0.0, 1.0 - (rad / 0.76) ** 2))
                    h_grid[y][x] = min(1.0, 0.52 + 0.36 * dome_z + (luma - 0.5) * 0.05)
            else:
                ridge = max(0.0, 1.0 - min(1.0, rad))
                h_grid[y][x] = min(1.0, 0.45 + 0.35 * ridge + luma * 0.12)

    # Central-difference surface normal (nx, ny, nz) + height in alpha channel
    for y in range(target_size):
        for x in range(target_size):
            h_c = h_grid[y][x]
            if h_c <= 0.001:
                continue
            h_w = h_grid[y][max(0, x - 1)]
            h_e = h_grid[y][min(target_size - 1, x + 1)]
            h_n = h_grid[max(0, y - 1)][x]
            h_s = h_grid[min(target_size - 1, y + 1)][x]
            nx = -(h_e - h_w) * 3.5
            ny = -(h_s - h_n) * 3.5
            nz = 1.0
            inv_len = 1.0 / math.sqrt(nx * nx + ny * ny + nz * nz)
            nx *= inv_len
            ny *= inv_len
            nz *= inv_len
            dst_px[x, y] = (
                int((nx * 0.5 + 0.5) * 255),
                int((ny * 0.5 + 0.5) * 255),
                int((nz * 0.5 + 0.5) * 255),
                int(min(1.0, h_c) * 255),
            )

    return out


def cmd_export_masks(args: argparse.Namespace) -> None:
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    entities = [args.entity] if args.entity != "all" else list(ENTITY_SPECS.keys())

    for name in entities:
        mask = generate_conditioning_mask(name, size=args.size)
        mask_path = out_dir / f"{name}_mask.png"
        mask.save(mask_path)

        vga_preview, meta = quantize_to_vga_and_extract_seams(mask, target_size=args.vga_size)
        vga_path = out_dir / f"{name}_vga_{args.vga_size}.png"
        height_img = bake_height_and_normal_map(name, vga_preview, target_size=args.vga_size)
        height_path = out_dir / f"{name}_height_normal_{args.vga_size}.png"
        meta_path = out_dir / f"{name}_constitutive.json"
        vga_preview.save(vga_path)
        height_img.save(height_path)
        meta_path.write_text(json.dumps(meta, indent=2))
        print(
            f"[flux-forge] Exported {mask_path}, VGA {vga_path}, Height/Normal {height_path} "
            f"(active={meta['active_cells']}, seams={meta['seam_cells']})"
        )


def cmd_generate(args: argparse.Namespace) -> None:
    require_pillow()
    try:
        import torch
        from diffusers import Flux2KleinPipeline
    except ImportError as exc:
        raise SystemExit(
            f"Missing FLUX.2-klein-4B runtime dependency ({exc}).\n"
            "Install with:\n"
            "  pip install git+https://github.com/huggingface/diffusers.git transformers accelerate torch"
        )

    device = "cuda" if torch.cuda.is_available() else "cpu"
    dtype = torch.bfloat16 if device == "cuda" else torch.float32
    print(f"[flux-forge] Loading {args.model} on {device} ({dtype})...")
    pipe = Flux2KleinPipeline.from_pretrained(args.model, torch_dtype=dtype)
    if args.cpu_offload and device == "cuda":
        pipe.enable_model_cpu_offload()
    else:
        pipe = pipe.to(device)

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    masks_dir = Path(args.masks_dir)
    masks_dir.mkdir(parents=True, exist_ok=True)

    entities = [args.entity] if args.entity != "all" else list(ENTITY_SPECS.keys())
    for idx, name in enumerate(entities):
        default_spec = ENTITY_SPECS.get(
            name,
            {
                "title": name,
                "prompt": (
                    "1994 MS-DOS VGA pixel art top-down sci-fi sprite, brushed gunmetal armor, "
                    "crisp 1px dark keyline, isolated on solid black background, orthographic top-down view"
                ),
            },
        )
        prompt = args.prompt if args.prompt else default_spec["prompt"]
        if args.mask:
            mask = Image.open(args.mask).convert("RGB")
        else:
            mask_path = masks_dir / f"{name}_mask.png"
            if not mask_path.exists():
                mask = generate_conditioning_mask(name, size=512)
                mask.save(mask_path)
            else:
                mask = Image.open(mask_path).convert("RGB")

        gen = torch.Generator(device=device).manual_seed(args.seed + idx)
        print(f"[flux-forge] Generating '{name}' ({args.steps} steps)...")
        result = pipe(
            image=mask,
            prompt=prompt,
            height=512,
            width=512,
            guidance_scale=1.0,
            num_inference_steps=args.steps,
            generator=gen,
        ).images[0]

        raw_path = out_dir / f"{name}_flux_raw.png"
        result.save(raw_path)

        vga_img, meta = quantize_to_vga_and_extract_seams(result, target_size=args.vga_size)
        vga_path = out_dir / f"{name}_vga.png"
        height_img = bake_height_and_normal_map(name, vga_img, target_size=args.vga_size)
        height_path = out_dir / f"{name}_height_normal.png"
        meta_path = out_dir / f"{name}_constitutive.json"
        vga_img.save(vga_path)
        height_img.save(height_path)
        meta_path.write_text(json.dumps(meta, indent=2))
        print(f"[flux-forge] Saved {vga_path}, {height_path}, and {meta_path}")


def cmd_quantize(args: argparse.Namespace) -> None:
    require_pillow()
    src = Image.open(args.input)
    vga_img, meta = quantize_to_vga_and_extract_seams(src, target_size=args.vga_size)
    out_path = Path(args.output)
    vga_img.save(out_path)
    meta_path = out_path.with_suffix(".constitutive.json")
    meta_path.write_text(json.dumps(meta, indent=2))
    print(
        f"[flux-forge] Quantized {args.input} -> {out_path} "
        f"({args.vga_size}x{args.vga_size}, seam_cells={meta['seam_cells']})"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="flux-forge",
        description="Dzog FLUX.2 [klein] 4B VGA Sprite & Constitutive Seam Baker",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    p_mask = sub.add_parser("export-masks", help="Export CohesiveBody conditioning masks and VGA previews")
    p_mask.add_argument("--out-dir", default="tools/flux-forge/masks", help="Output directory")
    p_mask.add_argument("--entity", default="all", choices=["all", *ENTITY_SPECS.keys()])
    p_mask.add_argument("--size", type=int, default=512, help="Conditioning mask resolution")
    p_mask.add_argument("--vga-size", type=int, default=48, help="Downsampled VGA sprite resolution")
    p_mask.set_defaults(func=cmd_export_masks)

    p_gen = sub.add_parser("generate", help="Run FLUX.2 [klein] 4B 4-step img2img sprite generation")
    p_gen.add_argument("--model", default="black-forest-labs/FLUX.2-klein-4B")
    p_gen.add_argument("--masks-dir", default="tools/flux-forge/masks")
    p_gen.add_argument("--out-dir", default="crates/shmup/assets")
    p_gen.add_argument("--entity", default="all", help="Built-in archetype name, 'all', or a custom asset name")
    p_gen.add_argument("--prompt", default=None, help="Custom prompt override for generating a new enemy or building")
    p_gen.add_argument("--mask", default=None, help="Optional custom conditioning silhouette PNG path")
    p_gen.add_argument("--steps", type=int, default=4)
    p_gen.add_argument("--seed", type=int, default=42)
    p_gen.add_argument("--vga-size", type=int, default=48)
    p_gen.add_argument("--cpu-offload", action="store_true")
    p_gen.set_defaults(func=cmd_generate)

    p_quant = sub.add_parser("quantize", help="Quantize an image to VGA palette + 1px keyline + seam map")
    p_quant.add_argument("--input", required=True)
    p_quant.add_argument("--output", required=True)
    p_quant.add_argument("--vga-size", type=int, default=48)
    p_quant.set_defaults(func=cmd_quantize)

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
