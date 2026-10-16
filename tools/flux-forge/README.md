# `dzog-flux-forge` — Standalone CLI Asset, Heightmap & Constitutive Baker

`tools/flux-forge` is a standalone ancillary CLI tool that bridges **Black Forest Labs' [`flux-3-image-klein`](https://huggingface.co/black-forest-labs/flux-3-image-klein)** (16-step distilled flow-map transformer + Qwen3-VL-4B-Instruct text encoder) with the **Dzog** field-physics engine.

For **16 GB cards (RTX 4080) or smaller**, `flux-forge` defaults to the dynamic 8-bit SDNQ quantization ([`OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_8bit`](https://huggingface.co/OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_8bit), ~12 GB VRAM at `512×512`) and also supports 4-bit SDNQ ([`OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_4bit`](https://huggingface.co/OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_4bit), ~8.5 GB VRAM for 8–12 GB GPUs) alongside full `bf16` ([`black-forest-labs/flux-3-image-klein`](https://huggingface.co/black-forest-labs/flux-3-image-klein), 24 GB+ GPUs). Any legacy `FLUX.2` cached weights are automatically purged.

Rather than running neural inference inside the 60 Hz simulation loop (which would violate frame-time determinism and smear physical cleavage seams), `flux-forge` conditions `flux-3-image-klein` on exact `CohesiveBody` structural masks and bakes:
1. **1994 VGA Sprite Textures (`*_vga.png`)**: `48×48` 5-bit palette-quantized sprites with 1px dark charcoal keylines.
2. **4-Channel Height & Surface-Normal Maps (`*_height_normal.png`)**: Packed `(nx, ny, nz, elevation_h)` maps for 2.5D horizon shadows and 3D falling-wreckage roof deflection.
3. **Constitutive Cleavage Seam Maps (`*_constitutive.json`)**: Maps dark armor panel seams in the sprite directly to pre-stressed local cohesion values (`C_0 = 0.84`), so ships and domes physically fracture along their visual armor plates.

## Quantization Tiers (`--quant`)

| `--quant` | HuggingFace Checkpoint | Peak VRAM (`512×512`) | Target GPU Tier |
| :--- | :--- | :--- | :--- |
| `int8` *(default)* | [`OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_8bit`](https://huggingface.co/OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_8bit) | ~12 GB (15.4 GB at 2K) | **RTX 4080 (16 GB)** / 16 GB GPUs |
| `int4` | [`OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_4bit`](https://huggingface.co/OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_4bit) | ~8.5 GB (11.5 GB at 2K) | **8 GB – 12 GB GPUs** (RTX 3070 / 4070) |
| `bf16` | [`black-forest-labs/flux-3-image-klein`](https://huggingface.co/black-forest-labs/flux-3-image-klein) | ~24–29 GB | 24 GB+ GPUs (RTX 4090 / 5090) |
| `auto` | Auto-selects `int4` (<14 GB), `int8` (14–20 GB) | Adaptive | Automatic |

## Example CLI Usage

```bash
# 1. Export deterministic CohesiveBody conditioning masks, VGA previews, heightmaps & seam JSONs
python3 tools/flux-forge/flux_forge.py export-masks --out-dir tools/flux-forge/masks --vga-size 48

# 2. Generate all built-in sprites on a 16 GB RTX 4080 (defaults to --quant int8, 16 flow-map steps)
python3 tools/flux-forge/flux_forge.py generate \
  --quant int8 \
  --masks-dir tools/flux-forge/masks \
  --out-dir crates/shmup/assets \
  --steps 16

# 3. Generate on an 8 GB - 12 GB GPU using 4-bit dynamic SDNQ quantization
python3 tools/flux-forge/flux_forge.py generate \
  --quant int4 \
  --masks-dir tools/flux-forge/masks \
  --out-dir crates/shmup/assets

# 4. Generate a new custom enemy ship from a scene caption and conditioning mask
python3 tools/flux-forge/flux_forge.py generate \
  --quant int8 \
  --entity heavy_bomber \
  --mask tools/flux-forge/masks/twin_gunship_mask.png \
  --prompt "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down heavy twin-boom stealth bomber sprite in the style of Raptor: Call of the Shadows, dark slate armor plates, dual forward orange plasma bays, recessed charcoal cleavage seams, 1px dark keyline, solid black background." \
  --vga-size 48 \
  --out-dir crates/shmup/assets

# 5. Purge any legacy FLUX.2 model caches from disk
python3 tools/flux-forge/flux_forge.py purge-old

# 6. Quantize any custom PNG into a 48x48 VGA sprite + constitutive cleavage seam map
python3 tools/flux-forge/flux_forge.py quantize \
  --input my_ship.png \
  --output crates/shmup/assets/custom_ship_vga.png \
  --vga-size 48
```
