# `dzog-flux-forge` — Standalone CLI Asset, Heightmap & Constitutive Baker

`tools/flux-forge` is a standalone ancillary CLI tool that bridges **Black Forest Labs' [`FLUX.2-klein-4B`](https://huggingface.co/black-forest-labs/FLUX.2-klein-4B)** (4-step rectified flow transformer, Apache 2.0, ~13 GB VRAM) with the **Dzog** field-physics engine.

Rather than running neural inference inside the 60 Hz simulation loop (which would violate frame-time determinism and smear physical cleavage seams), `flux-forge` conditions `FLUX.2-klein-4B` on exact `CohesiveBody` structural masks and bakes:
1. **1994 VGA Sprite Textures (`*_vga.png`)**: `48×48` 5-bit palette-quantized sprites with 1px dark charcoal keylines.
2. **4-Channel Height & Surface-Normal Maps (`*_height_normal.png`)**: Packed `(nx, ny, nz, elevation_h)` maps for 2.5D horizon shadows and 3D falling-wreckage roof deflection.
3. **Constitutive Cleavage Seam Maps (`*_constitutive.json`)**: Maps dark armor panel seams in the sprite directly to pre-stressed local cohesion values (`C_0 = 0.84`), so ships and domes physically fracture along their visual armor plates.

## Example CLI Usage

```bash
# 1. Export deterministic CohesiveBody conditioning masks, VGA previews, heightmaps & seam JSONs
python3 tools/flux-forge/flux_forge.py export-masks --out-dir tools/flux-forge/masks --vga-size 48

# 2. Generate all built-in sprites on local CUDA GPU with FLUX.2 [klein] 4B (4 inference steps)
python3 tools/flux-forge/flux_forge.py generate \
  --model black-forest-labs/FLUX.2-klein-4B \
  --masks-dir tools/flux-forge/masks \
  --out-dir crates/shmup/assets \
  --steps 4

# 3. Generate a new custom enemy ship from a prompt and conditioning mask
python3 tools/flux-forge/flux_forge.py generate \
  --entity heavy_bomber \
  --mask tools/flux-forge/masks/twin_gunship_mask.png \
  --prompt "1994 MS-DOS VGA pixel art top-down heavy twin-boom stealth bomber sprite, dark slate armor, dual orange plasma bays, 1px dark keyline, solid black background" \
  --vga-size 48 \
  --out-dir crates/shmup/assets

# 4. Generate a new custom ground building (bakes VGA sprite + height/normal map + cleavage seams)
python3 tools/flux-forge/flux_forge.py generate \
  --entity radar_bunker \
  --mask tools/flux-forge/masks/geodesic_dome_mask.png \
  --prompt "1994 MS-DOS VGA pixel art top-down armored concrete radar bunker building, rotating dish, steel rivets, top-left lighting, solid black background" \
  --vga-size 48 \
  --out-dir crates/shmup/assets

# 5. Quantize any custom PNG into a 48x48 VGA sprite + constitutive cleavage seam map
python3 tools/flux-forge/flux_forge.py quantize \
  --input my_ship.png \
  --output crates/shmup/assets/custom_ship_vga.png \
  --vga-size 48
```
