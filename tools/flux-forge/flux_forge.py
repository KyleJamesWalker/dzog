#!/usr/bin/env python3
"""
dzog-flux-forge: Standalone CLI tool for conditioning `black-forest-labs/flux-3-image-klein`
(and its 8-bit/4-bit SDNQ quantizations for <=16 GB GPUs such as the RTX 4080) on Dzog
`CohesiveBody` structural silhouettes to generate sharp 1994 VGA (*Raptor: Call of the Shadows*
style) sprite atlases and inverse constitutive material maps (Albedo + Normal/Elevation + Cleavage Seams).

Usage examples:
  # 1. Export deterministic CohesiveBody conditioning masks (no GPU/torch required)
  python3 tools/flux-forge/flux_forge.py export-masks --out-dir tools/flux-forge/masks

  # 2. Generate VGA retro sprites using FLUX 3 Image Klein INT8 SDNQ (16-step flow-map on 16 GB RTX 4080)
  python3 tools/flux-forge/flux_forge.py generate --quant int8 --masks-dir tools/flux-forge/masks --out-dir crates/shmup/assets

  # 3. Purge any cached legacy FLUX.2 weights from HuggingFace caches
  python3 tools/flux-forge/flux_forge.py purge-old

  # 4. Post-process any sprite into VGA 256-color palette + 1px keyline + RON constitutive seam map
  python3 tools/flux-forge/flux_forge.py quantize --input sprite.png --output vga_sprite.png
"""

from __future__ import annotations

import argparse
import json
import math
import os
import shutil
from pathlib import Path
from typing import Dict, List, Tuple

try:
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:
    Image = None  # Handled gracefully with clear CLI instructions


FLUX3_QUANTS: Dict[str, str] = {
    "int8": "OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_8bit",
    "int4": "OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_4bit",
    "bf16": "black-forest-labs/flux-3-image-klein",
}

LEGACY_FLUX2_PATTERNS: Tuple[str, ...] = (
    "models--black-forest-labs--FLUX.2*",
    "models--*flux-2*",
    "models--*flux2*",
)


ENTITY_SPECS: Dict[str, Dict[str, str]] = {
    "player_raptor": {
        "title": "Tri-Lobe Raptor Interceptor",
        "prompt": (
            "A premium neo-retro arcade top-down military sci-fi fighter aircraft sprite, inspired by Raptor: Call "
            "of the Shadows and Strikers 1945. Orthographic straight top-down view centered on solid pure black background "
            "(#000000). The hull is constructed from faceted titanium-gunmetal composite armor with crisp beveled facets, "
            "recessed dark cleavage panel grooves, and exposed brass/copper intake cooling vents. Swept triangular delta "
            "wings with bold crimson-red recognition chevrons, forward-facing dual autocannon muzzles integrated flush "
            "into the wing roots, and an amber-tinted polarized glass bubble cockpit canopy with illuminated holographic "
            "flight avionics. Clean, cold thruster nozzles at the rear with no exhaust flame. Strong top-left directional "
            "lighting casting sharp metallic specular highlights and deep contrasting shadows across panel lines. "
            "Crisp, razor-sharp pixel edges with a solid dark charcoal 1px perimeter outline. Zero exhaust plumes, "
            "zero background artifacts, isolated craft."
        ),
    },
    "twin_gunship": {
        "title": "Twin-Nacelle Heavy Strike Gunship",
        "prompt": (
            "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down heavy twin-fuselage "
            "strike gunship sprite in the style of Raptor: Call of the Shadows. Centered in an orthographic top-down "
            "view on a solid pure black background, the vessel has two parallel armored engine nacelles flanking a "
            "recessed central command pod connected by heavy wingbox struts. The hull is clad in weathered dark "
            "gunmetal steel plates with industrial rivets, ribbed forward air intakes, dual glowing orange plasma "
            "cannon barrels on the nose, and recessed charcoal structural cleavage seams. Lit from the top-left with "
            "stepped 5-bit VGA specular highlights and a crisp 1-pixel dark keyline; keep the exact twin-nacelle "
            "silhouette and solid black background."
        ),
    },
    "chladni_matriarch": {
        "title": "Chladni Swarm Carrier Cruiser",
        "prompt": (
            "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down alien command "
            "cruiser sprite in the style of Raptor: Call of the Shadows. Centered in an orthographic top-down view "
            "against a solid pure black background, the biomechanical hull has a broad elliptical upper carapace in "
            "burnished bronze and deep violet armor plates, with twin forward tuning-fork resonance mandibles "
            "extending downward around a glowing cyan acoustic core. Deep charcoal panel grooves trace the structural "
            "lobes under crisp top-left directional pixel lighting with a 1-pixel dark outline; preserve the exact "
            "mandible and carapace silhouette on a solid black background."
        ),
    },
    "geodesic_dome": {
        "title": "Geodesic Fuel Dome",
        "prompt": (
            "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down spherical geodesic "
            "industrial fuel storage dome building in the style of Raptor: Call of the Shadows. Centered in an "
            "orthographic top-down view on a solid pure black background, the hemispherical steel dome is divided by "
            "a triangulated dark charcoal truss grid and bolted armor seams, with four cardinal pipeline manifold "
            "flanges at north, south, east, and west. Strong top-left directional sunlight creates a bright specular "
            "highlight on the upper-left dome facet and deep shaded lower-right panels with a 1-pixel dark keyline; "
            "preserve the circular footprint and solid black background."
        ),
    },
    "biodome_reactor": {
        "title": "Biodome Exciton Reactor",
        "prompt": (
            "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down circular glass "
            "biodome exciton reactor building in the style of Raptor: Call of the Shadows. Centered in an "
            "orthographic top-down view on a solid pure black background, a thick octagonal reinforced concrete "
            "containment collar surrounds a domed silver cross-hatch glass roof glazing over a glowing emerald-green "
            "bioluminescent crystalline core. Top-left directional lighting casts crisp specular glints across the "
            "glass struts and dark charcoal structural seams with a 1-pixel dark keyline; keep the exact circular "
            "silhouette and solid black background."
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
            if a > 32 and bright > 0.03:
                occupied[y][x] = True
                luma[y][x] = bright
            else:
                pixels[x, y] = (0, 0, 0, 0)

    # Flood-fill exterior background from image borders so dark interior canopies/seams stay solid
    exterior = [[False] * target_size for _ in range(target_size)]
    queue: List[Tuple[int, int]] = []
    for i in range(target_size):
        for bx, by in ((i, 0), (i, target_size - 1), (0, i), (target_size - 1, i)):
            if not occupied[by][bx] and not exterior[by][bx]:
                exterior[by][bx] = True
                queue.append((bx, by))
    while queue:
        qx, qy = queue.pop()
        for nx, ny in ((qx - 1, qy), (qx + 1, qy), (qx, qy - 1), (qx, qy + 1)):
            if 0 <= nx < target_size and 0 <= ny < target_size and not occupied[ny][nx] and not exterior[ny][nx]:
                exterior[ny][nx] = True
                queue.append((nx, ny))

    for y in range(target_size):
        for x in range(target_size):
            if not exterior[y][x] and not occupied[y][x]:
                occupied[y][x] = True
                pixels[x, y] = (24, 32, 44, 255)
                luma[y][x] = 0.12

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


def purge_legacy_flux2_models(verbose: bool = True) -> List[Path]:
    """Delete any cached legacy FLUX.2 checkpoints from HuggingFace Hub caches and local weights dirs."""
    search_roots = [
        Path(os.environ.get("HF_HUB_CACHE", Path.home() / ".cache" / "huggingface" / "hub")),
        Path(os.environ.get("HF_HOME", Path.home() / ".cache" / "huggingface")) / "hub",
        Path("/mnt/storage/models/huggingface/hub"),
        Path("weights"),
    ]
    removed: List[Path] = []
    seen: set[Path] = set()
    for root in search_roots:
        if not root.exists():
            continue
        for pattern in LEGACY_FLUX2_PATTERNS:
            for match in root.glob(pattern):
                resolved = match.resolve()
                if resolved in seen or not match.exists():
                    continue
                seen.add(resolved)
                if match.is_dir():
                    shutil.rmtree(match)
                else:
                    match.unlink()
                removed.append(match)
                if verbose:
                    print(f"[flux-forge] Deleted legacy FLUX.2 model cache: {match}")
    return removed


def cmd_purge_old(args: argparse.Namespace) -> None:
    removed = purge_legacy_flux2_models(verbose=True)
    if not removed:
        print("[flux-forge] Confirmed clean: no legacy FLUX.2 model caches found on disk.")


def resolve_flux3_model_id(model_arg: str | None, quant: str, device: str) -> str:
    """Resolve the best FLUX 3 Image Klein checkpoint for the target GPU VRAM tier."""
    if model_arg:
        if "FLUX.2" in model_arg or "flux-2" in model_arg.lower():
            raise SystemExit(
                f"Error: Legacy model '{model_arg}' is deprecated and removed. "
                f"Use FLUX 3 Image Klein (`--quant int8` -> {FLUX3_QUANTS['int8']})."
            )
        return model_arg

    if quant == "auto":
        if device == "cuda":
            try:
                import torch

                vram_gb = torch.cuda.get_device_properties(0).total_memory / (1024**3)
                if vram_gb < 14.0:
                    return FLUX3_QUANTS["int4"]
                if vram_gb <= 20.0:
                    return FLUX3_QUANTS["int8"]
            except Exception:
                pass
        return FLUX3_QUANTS["int8"]

    return FLUX3_QUANTS[quant]


def cmd_generate(args: argparse.Namespace) -> None:
    require_pillow()
    purge_legacy_flux2_models(verbose=True)

    try:
        import torch
        from diffusers import ModularPipeline
    except ImportError as exc:
        raise SystemExit(
            f"Missing FLUX 3 Image Klein runtime dependency ({exc}).\n"
            "Install with:\n"
            "  pip install git+https://github.com/huggingface/diffusers.git sdnq transformers accelerate torch"
        )

    device = "cuda" if torch.cuda.is_available() else "cpu"
    dtype = torch.bfloat16 if device == "cuda" else torch.float32
    model_id = resolve_flux3_model_id(args.model, args.quant, device)

    if "sdnq" in model_id.lower():
        try:
            import sdnq  # noqa: F401 — registers dynamic INT8/INT4 quantized weights with Diffusers
        except ImportError as exc:
            raise SystemExit(
                f"Missing `sdnq` package required for quantized FLUX 3 checkpoint '{model_id}' ({exc}).\n"
                "Install with: pip install 'sdnq>=0.2.8'"
            )

    print(f"[flux-forge] Loading FLUX 3 Image Klein ({model_id}, quant={args.quant}) on {device} ({dtype})...")
    pipe = ModularPipeline.from_pretrained(model_id)
    pipe.load_components(dtype=dtype)
    if args.cpu_offload and device == "cuda" and hasattr(pipe, "enable_model_cpu_offload"):
        pipe.enable_model_cpu_offload()
    else:
        pipe.to(device)

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
                    "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down sci-fi "
                    "sprite in the style of Raptor: Call of the Shadows. Centered in an orthographic top-down view "
                    "on a solid pure black background, clad in brushed gunmetal armor plates with recessed charcoal "
                    "structural cleavage seams, top-left directional lighting, and a crisp 1-pixel dark keyline; "
                    "preserve the exact silhouette and solid black background."
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

        gen = torch.Generator().manual_seed(args.seed + idx)
        print(f"[flux-forge] Generating '{name}' ({args.steps} flow-map steps, model={model_id})...")
        result = pipe(
            image=mask,
            prompt=prompt,
            height=512,
            width=512,
            num_inference_steps=args.steps,
            generator=gen,
            output="images",
        )[0]

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

        bin_path = out_dir / f"{name}_vga.bin"
        bin_bytes = bytearray()
        rgba_px = vga_img.convert("RGBA").load()
        for y in range(args.vga_size):
            for x in range(args.vga_size):
                bin_bytes.extend(rgba_px[x, y])
        bin_path.write_bytes(bin_bytes)

        print(f"[flux-forge] Saved {vga_path}, {height_path}, {meta_path}, and {bin_path}")


def cmd_quantize(args: argparse.Namespace) -> None:
    require_pillow()
    src = Image.open(args.input)
    vga_img, meta = quantize_to_vga_and_extract_seams(src, target_size=args.vga_size)
    out_path = Path(args.output)
    vga_img.save(out_path)
    entity_hint = out_path.stem.replace("_vga", "")
    height_img = bake_height_and_normal_map(entity_hint, vga_img, target_size=args.vga_size)
    height_path = out_path.with_name(f"{entity_hint}_height_normal.png")
    height_img.save(height_path)
    meta_path = out_path.with_name(f"{entity_hint}_constitutive.json")
    meta_path.write_text(json.dumps(meta, indent=2))

    # Also bake uncompressed RGBA binary for instant zero-dependency include_bytes! embedding
    bin_path = out_path.with_name(f"{entity_hint}_vga.bin")
    bin_bytes = bytearray()
    rgba_px = vga_img.convert("RGBA").load()
    for y in range(args.vga_size):
        for x in range(args.vga_size):
            bin_bytes.extend(rgba_px[x, y])
    bin_path.write_bytes(bin_bytes)

    print(
        f"[flux-forge] Quantized {args.input} -> {out_path}, {height_path}, {meta_path}, {bin_path} "
        f"({args.vga_size}x{args.vga_size}, active={meta['active_cells']}, seam_cells={meta['seam_cells']})"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="flux-forge",
        description="Dzog FLUX 3 Image Klein (INT8/INT4 SDNQ & BF16) VGA Sprite & Constitutive Seam Baker",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    p_mask = sub.add_parser("export-masks", help="Export CohesiveBody conditioning masks and VGA previews")
    p_mask.add_argument("--out-dir", default="tools/flux-forge/masks", help="Output directory")
    p_mask.add_argument("--entity", default="all", choices=["all", *ENTITY_SPECS.keys()])
    p_mask.add_argument("--size", type=int, default=512, help="Conditioning mask resolution")
    p_mask.add_argument("--vga-size", type=int, default=48, help="Downsampled VGA sprite resolution")
    p_mask.set_defaults(func=cmd_export_masks)

    p_gen = sub.add_parser(
        "generate",
        help="Run FLUX 3 Image Klein 16-step flow-map img2img sprite generation (defaults to INT8 SDNQ for <=16 GB GPUs)",
    )
    p_gen.add_argument(
        "--quant",
        default="int8",
        choices=["int8", "int4", "bf16", "auto"],
        help="Quantization tier: 'int8' (best for 16 GB RTX 4080, ~12 GB VRAM), 'int4' (<=12 GB GPUs, ~9 GB VRAM), 'bf16' (24 GB+ GPUs)",
    )
    p_gen.add_argument(
        "--model",
        default=None,
        help="Optional explicit HuggingFace repo ID override (defaults to --quant selection)",
    )
    p_gen.add_argument("--masks-dir", default="tools/flux-forge/masks")
    p_gen.add_argument("--out-dir", default="crates/shmup/assets")
    p_gen.add_argument("--entity", default="all", help="Built-in archetype name, 'all', or a custom asset name")
    p_gen.add_argument("--prompt", default=None, help="Custom scene caption override for generating a new enemy or building")
    p_gen.add_argument("--mask", default=None, help="Optional custom conditioning silhouette PNG path")
    p_gen.add_argument("--steps", type=int, default=16, help="Flow-map sampling steps (16 for distilled FLUX 3 Klein)")
    p_gen.add_argument("--seed", type=int, default=42)
    p_gen.add_argument("--vga-size", type=int, default=48)
    p_gen.add_argument("--cpu-offload", action="store_true")
    p_gen.set_defaults(func=cmd_generate)

    p_purge = sub.add_parser("purge-old", help="Delete any cached legacy FLUX.2 checkpoints from disk")
    p_purge.set_defaults(func=cmd_purge_old)

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
