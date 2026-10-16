# Shmup Architecture & Field-Design Insights

This document records the core architectural decisions, mathematical trade-offs, and implementation patterns powering **`crates/shmup`**.

---

## 1. Why a 2.5D Topographic Field Beats a Full 3D Voxel Grid

A central architectural question for a vertical shoot-'em-up with airborne squadrons, tall ground structures, directional shadows, and falling wreckage is whether to step the governing PDEs on a 3D voxel grid (`Nx × Ny × Nz`) or a 2.5D topographic lattice (`Nx × Ny` with continuous elevation $h(\mathbf{x})$).

### The Math Against Uniform 3D Voxels (`240 × 135 × 64`)
1. **$80\times$ Compute & Bandwidth Penalty**: Adding 64 vertical slices multiplies cell count from $32,400$ to $2,073,600$ ($64\times$), widens the Laplacian stencil from 5 points to 7 points, and tightens the Courant–Friedrichs–Lewy (CFL) time-step bound by $\sqrt{3/2} \approx 1.22\times$. Over $90\%$ of those voxels represent empty air above or between aircraft.
2. **Spherical Wave Attenuation ($1/r$ vs. $1/\sqrt{r}$)**: In 3D, wave amplitude decays as $1/r$ into the vertical hemisphere rather than $1/\sqrt{r}$ in a 2D plane, starving horizontal wave-matter agency and pilot-wave guidance ($-\nabla Q$) across the arena.
3. **Coarser Vertical Precision**: A 64-slice grid quantizes altitude into stair-stepped `1/64` voxel increments, whereas a 2D `f32` heightfield $h(x, y) \in [0, 1]$ provides continuous floating-point vertical precision at $1/64\text{th}$ the memory cost.

### The 2.5D Field Solution (`dzog_core::MaterialField`)
Instead of 3D voxels, `MaterialField` stores two 2D topographic channels alongside $(\rho, K, \gamma, C)$:
- **Surface Elevation $h(x, y) \in [0, 1]$**: Encodes the vertical height of ground domes, reactors, turrets, and terrain.
- **Slope Normal $-\nabla h(x, y)$ (`MaterialField::elevation_and_gradient_at`)**: Central finite differences yield the 3D surface slope in **$O(1)$** time without iterating over building lists.
- **Single-Pass Horizon Shadows $h_{\text{shadow}}(x, y)$ (`MaterialField::recompute_horizon_shadows`)**: A single **$O(\text{cells})$** dynamic-programming sweep along the solar vector propagates cast shadows across the entire arena:
  $$h_{\text{shadow}}(x, y) = \max\!\left(h(x, y), \; h_{\text{shadow}}(x + s_x, y - s_y) - \Delta h_{\text{sun}}\right)$$
  Any sub-pixel with $h_{\text{shadow}}(x, y) > h(x, y)$ is attenuated by shadow depth, eliminating $O(N_{\text{buildings}} \times \text{ray\_steps})$ per-object shadow raymarching.

---

## 2. Decoupling Physics Resolution from Perceptual Texel Density

In a naive grid renderer where 1 physics cell equals 1 screen pixel, a ship of radius $6.5$ cells spans only $13 \times 13$ pixels—insufficient to resolve swept wings, canopies, turret barrels, or progressive armor fractures.

**Architecture**:
- **Physics Lattice (`240 × 135`, 16:9 aspect ratio)**: Locks CFL stability and keeps WGPU FDTD + zero-readback body reduction under $0.5\text{ ms/frame}$ (and $60\text{ FPS}$ on single-threaded `wasm32`).
- **Sub-Cell Perceptual Canvas (`960 × 540`, `RENDER_SCALE = 4`)**: Renders at $4\times$ sub-cell supersampling (an exact $2\times$ integer divisor of `1920 × 1080` and $4\times$ divisor of `3840 × 2160` 4K).
- Ships and ground domes span $40 \times 40$ to $60 \times 60$ sub-pixels with 1px dark charcoal keylines, 5-bit VGA palette ramps, and bilinearly interpolated field sampling.
- **Bit-Avalanched Spatial Hashing**: Ground and water pixel grain must avalanche high bits downward (`h ^= h >> 13; h *= 1274126177; (h >> 16) & 7`) before masking; taking low bits `& 7` of linear combinations `(ax + by)` produces periodic diagonal wave striping (`(x - y) mod 8`).

---

## 3. Topological Hulls, Jagged Fracturing, Enemy Bumping & Field-Driven Explosions

### A. Multi-Stage Progressive Fracturing & Sub-Cell Jagged Fissures (`CohesiveBody::inflict_progressive_fracture`)
- **Player Tri-Lobe Hull**: Replaces integer hitpoints (`hp: 3`) with three physical structural lobes (Port Wing, Central Core, Starboard Wing). Losing a wing physically removes its muzzle and reduces thruster authority until regrown via **Exciton Annealing** ($\Delta C > 0$) from captured `PowerCube` canisters.
- **Enemy Hulls (`TwinGunship`, `ChladniMatriarch`, `SectorDreadnought`)**:
  - Off-center kinetic bullet hits impart punchy backward and lateral recoil ($\Delta \mathbf{P} = \mathbf{J}$) and rotational torque ($\Delta L_z = (\mathbf{r} \times \mathbf{J})_z$).
  - Initial hits carve internal fracture waveguides (`0.22 <= C < 0.68`). Repeated strikes near existing cracks deepen surrounding cohesion toward cavitation ($C \to 0$) and branch secondary shear seams.
  - Inside fractured zones (`C < 0.72`), sub-pixel rendering evaluates a dual-branch fracture ridge function so damage appears as 1px white-gold and molten-orange lightning veins framed by a charred black carbon-soot lip (`render/ships.rs`, `render/structures.rs`).
  - Connected-component Breadth-First Search (`split_connected_components`) severs wings and engine nacelles while the surviving core continues flying under asymmetrical thrust yaw.
- **Field-Coupled Enemy-to-Enemy Bumping (`combat.rs`, `field_game.rs`)**:
  - All active enemy vessels participate in the GPU `reduce_body.wgsl` force/torque reduction ($-\nabla p, -\nabla Q$) and resolve pairwise elastic + rotational collisions (`apply_contact_impulse`). Knocking an enemy ship into a neighbor transfers momentum, spins both hulls apart without overlapping, and inflicts sympathetic fractures on high-speed impacts.

### B. Reaction-Diffusion Fiery Explosions & Rolling Smoke (`luminescence.wgsl`, `render/mod.rs`)
Rather than spawning detached particle sprites for fire and smoke, explosions dogfood the GPU's **reaction-diffusion luminescence field $L(\mathbf{x}, t) \in [0, 4]$** (`luminescence.wgsl`):
$$L^{n+1} = e^{-\gamma_L \Delta t}\left(L^n + D \Delta t \, \nabla^2 L^n\right) + \Delta t \, S_{\text{blast}}$$
1. **Multi-Lobe Thermal Seeding (`inject_fiery_field_explosion`)**: Impacts and detonations seed a 5-lobe asymmetric thermal kernel into `solver.state.luminescence` alongside a moderate acoustic pressure pulse (`GaussianImpulse`). Damaged hulls (`fracture_hits > 0`) continuously bleed small thermal puffs ($L \approx 0.72\text{–}1.25$) as they move.
2. **Combustion $\to$ Smoke Transfer Function (`draw_field_fire_and_smoke`)**:
   - **Detonation Core ($L_{\text{eff}} > 2.85$)**: White-yellow hot core (`[255, 248, 184]`).
   - **Inner Flame ($2.05 < L_{\text{eff}} \le 2.85$)**: Bright golden-orange flame (`[252, 168, 32]`).
   - **Outer Flame ($0.92 < L_{\text{eff}} \le 2.05$)**: Turbulent orange-red and dark crimson flame tongues (`[232, 88, 20]` $\to$ `[148, 32, 16]`).
   - **Condensing Smoke Tail ($0.32 \le L_{\text{eff}} \le 0.92$)**: As $L(\mathbf{x}, t)$ diffuses outward ($\nabla^2 L$) and cools ($e^{-\gamma_L \Delta t}$), it condenses into billowing dark charcoal smoke clouds (`[22, 20, 24]`).

### C. Building Wreckage & Blast Craters (`GroundedWreckage` & `ScorchedCrater`)
When a ground installation (`GeodesicFuelDome`, `BiodomeReactor`, `FlakTurret`) is destroyed:
1. **Structural Rubble (`GroundedWreckage`)**: Cleaved building hull sections settle around the blast site with tapered scorched skid trails and lingering ember glow, scrolling downward with the terrain.
2. **Stepped Soot Craters (`ScorchedCrater`)**: Stamps a trig-free 3-ring jagged VGA soot pit (`0.20× / 0.36× / 0.58×` darkened terrain) with discrete 1px ember specks.

---

## 4. Dual-Layer Sky/Ground Ecology & Waveguide Chain-Detonations

1. **Substrate Differentiation (`canyon.rs`)**:
   - `HydroCanal` ($\rho = 0.85, K = 0.92$): Recessed waterways carrying patrol boats and splash wakes.
   - `WaveguidePipeline` ($\rho = 0.74, K = 1.82, c = 1.57$): Fast-phonon conduits linking ground domes and flak batteries.
   - `IndustrialPlatform` & `TerraBasalt`: Elevated concrete hangar decks and regolith terrain.
2. **Ordnance Stratification (`combat.rs`)**:
   - **Primary (`KineticBullet`)**: High-force airborne projectiles that shove, spin, and progressively fracture sky squadrons and ground targets.
   - **Alt-Fire (`AirToGroundMissile`)**: Twin finned missiles that fly *beneath* airborne enemies, home onto `GroundTarget` installations, and detonate with high seismic overpressure.
3. **Sympathetic Waveguide Surges**:
   - Destroying a pipeline-coupled `GeodesicFuelDome` or `BiodomeReactor` injects a `DirectionalWavePacket` along the `pipeline_group`, sympathetically fracturing connected turrets and reactors and awarding bonus credits (`$`).

---

## 5. Offline Diffusion Conditioning vs. Real-Time Determinism (`tools/flux-forge`)

Running a 5.6B-parameter flow-map transformer inside a $60\text{ Hz}$ ($16.6\text{ ms}$) frame loop violates frame-time budgets, introduces temporal pixel boiling, and smears exact topological cleavage seams.

**Hybrid Pipeline (`tools/flux-forge/flux_forge.py`)**:
1. **Orthographic Mask Export (`export-masks`)**: Generates deterministic conditioning silhouettes directly from `CohesiveBody` geometry.
2. **16-Step Flow-Map Conditioning (`generate`)**: Runs [`black-forest-labs/flux-3-image-klein`](https://huggingface.co/black-forest-labs/flux-3-image-klein) via Diffusers `ModularPipeline` offline/at asset-build time, defaulting to 8-bit SDNQ quantization (`OzzyGT/Flux_3_Image_Klein_sdnq_dynamic_8bit`, ~12 GB VRAM on 16 GB RTX 4080) or 4-bit SDNQ (`--quant int4`, ~8.5 GB VRAM on 8–12 GB GPUs).
3. **VGA Quantization, Seam Extraction & Heightmap Baking (`quantize`, `bake_height_and_normal_map`)**:
   - Quantizes sprites to 5-bit VGA palettes with 1px dark charcoal keylines.
   - Outputs raw 48×48 RGBA `.bin` files (`9,216` bytes) for direct compile-time `include_bytes!` consumption by `crates/shmup/src/render/ships.rs`.
   - Maps dark armor panel lines into pre-stressed local cohesion values ($C_0 = 0.84$) so hulls physically fracture along their visual seams.
   - Exports 4-channel `(nx, ny, nz, elevation_h)` maps (`*_height_normal.png`) for 2.5D lighting and roof deflection.

### Asset Integration Contract for Ships
Each ship renderer in `crates/shmup/src/render/ships.rs` projects the `CohesiveBody`'s local coordinate frame onto the sprite atlas:
- The sub-cell loop iterates over the bounding box of `ship.radius`.
- Normalized coordinates `(nx, ny) = ((lx - center) / radius, (ly - center) / radius)` map onto `(tx, ty)` in the 48×48 sprite buffer.
- `a >= 32` pixels render the baked FLUX 3 texture, while procedural fracture cracks (`coh < 0.68`), invulnerability blinks, and field shield glows modulate on top.

### Example CLI Usage (`tools/flux-forge/flux_forge.py`)

```bash
# 1. Export all built-in CohesiveBody conditioning masks, VGA sprites, height/normal maps & seam JSONs
python3 tools/flux-forge/flux_forge.py export-masks --out-dir tools/flux-forge/masks --vga-size 48

# 2. Generate all built-in enemy & building sprites on a 16 GB RTX 4080 using FLUX 3 Image Klein INT8 (16 steps)
python3 tools/flux-forge/flux_forge.py generate \
  --quant int8 \
  --masks-dir tools/flux-forge/masks \
  --out-dir crates/shmup/assets \
  --steps 16

# 3. Generate a new custom enemy ship from a custom scene caption and conditioning mask
python3 tools/flux-forge/flux_forge.py generate \
  --quant int8 \
  --entity heavy_bomber \
  --mask tools/flux-forge/masks/twin_gunship_mask.png \
  --prompt "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down heavy twin-boom stealth bomber sprite in the style of Raptor: Call of the Shadows, dark slate armor, dual orange plasma bays, 1px dark keyline, solid black background." \
  --vga-size 48 \
  --out-dir crates/shmup/assets

# 4. Generate a new custom ground building (with automatic 4-channel height/normal map + cleavage seams)
python3 tools/flux-forge/flux_forge.py generate \
  --quant int8 \
  --entity radar_bunker \
  --mask tools/flux-forge/masks/geodesic_dome_mask.png \
  --prompt "Convert the entire reference silhouette into a 1994 MS-DOS VGA pixel-art top-down armored concrete radar bunker building in the style of Raptor: Call of the Shadows, rotating dish, steel rivets, top-left lighting, solid black background." \
  --vga-size 48 \
  --out-dir crates/shmup/assets

# 5. Quantize any external artwork into a 48x48 VGA sprite + constitutive cleavage seam map
python3 tools/flux-forge/flux_forge.py quantize \
  --input custom_art.png \
  --output crates/shmup/assets/custom_art_vga.png \
  --vga-size 48
```

---

## 6. Module Organization & Isolated Toy Verification

```text
crates/shmup/src/
├── main.rs            # CLI entry (--windowed, --autopilot, --record, --screenshot, --isolate)
├── field_game.rs      # Core state (240x135 16:9 lattice), FDTD stepping, GPU force reduction
├── canyon.rs          # Scrolling ground substrate, canals, pipelines, 3D building height profiles
├── entities.rs        # Sky/ground actors, GroundedWreckage, ScorchedCrater, ProjectileKind
├── spawning.rs        # Escalating wave formations, gunship pincers, dreadnought command waves
├── combat.rs          # Force bullets, air-to-ground missiles, enemy bumping, fiery field blasts
├── autopilot.rs       # Autonomous combat & target-sweeping pilot
├── gui.rs             # Bevy borderless fullscreen (1080p/4K), FPS HUD, Gamepad, MP4 recorder
└── render/
    ├── mod.rs         # Cell-hoisted 4x (960x540) ground pass, craters, fire-to-smoke PDE pass
    ├── hud.rs         # Vertical side gauges, outlined $00000000 ticker, missile ordnance badge
    ├── shadows.rs     # 2.5D sky drop-shadows
    ├── ships.rs       # Player interceptor & enemy squadron VGA rendering with jagged cracks
    ├── structures.rs  # Domes, biodomes, turrets, patrol boats, canisters, and projectiles
    └── wreckage.rs    # Grounded building rubble and scorched blast trails
```

### Performance Profile & Hot-Path Optimizations (`cargo run -p shmup -- --bench`)
Profiling 300 frames of `step() + render_to_pixel_buffer()` at `960 × 540` sub-cell resolution revealed two hot paths that were optimized for a **4.7× total FPS gain** (`31.8 FPS` $\to$ `148.5 FPS`, `6.74 ms/frame`):
1. **Cell-Hoisted Terrain Evaluation (`render/mod.rs`)**: Hoisted `canyon_wall_bounds` to run once per lattice row (`135` rows) and `sample_ground_cell` to run once per lattice cell (`32,400` cells) instead of per sub-pixel (`518,400` sub-pixels), eliminating ~4.1 million trigonometric calls per frame.
2. **Lipschitz Interior Fast-Path (`check_subpixel_edge`)**: Because bilinear cohesion changes by at most `step` ($\approx 0.25$) over 1 sub-pixel, any texel with `cohesion >= 0.48` cannot border `cohesion < 0.15`, skipping 4-neighbor bilinear edge checks on ~90% of interior sprite pixels.

### Verification Workflow
1. **Unit & Physics Gate**: `cargo test --workspace` verifies tri-lobe ship cleaving/annealing, progressive enemy fracturing, enemy non-overlap collision bumping, and missile waveguide chain-detonation.
2. **Isolated Toy Scenarios (`cargo run -p shmup -- --isolate docs`)**: Renders isolated single-feature scenes (`toy_1_player_clean.png`, `toy_2_shadows_and_roof.png`, `toy_3_wreckage_and_splash.png`) to inspect visual changes without multi-wave combat clutter.
