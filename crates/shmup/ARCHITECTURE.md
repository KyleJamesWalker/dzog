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

## 3. Topological Hulls, Progressive Fracturing & 3D Falling Wreckage

### A. Multi-Stage Progressive Fracturing (`CohesiveBody::inflict_progressive_fracture`)
- **Player Tri-Lobe Hull**: Replaces integer hitpoints (`hp: 3`) with three physical structural lobes (Port Wing, Central Core, Starboard Wing). Losing a wing physically removes its muzzle and reduces thruster authority until regrown via **Exciton Annealing** ($\Delta C > 0$) from captured `PowerCube` canisters.
- **Enemy Hulls (`TwinGunship`, `ChladniMatriarch`, `SectorDreadnought`)**:
  - Off-center kinetic bullet hits impart both linear knockback ($\Delta \mathbf{P} = \mathbf{J}$) and rotational torque ($\Delta L_z = (\mathbf{r} \times \mathbf{J})_z$).
  - Initial hits carve internal fracture waveguides (`0.22 <= C < 0.68`). Repeated strikes near existing cracks deepen surrounding cohesion toward cavitation ($C \to 0$) and branch secondary shear seams.
  - Connected-component Breadth-First Search (`split_connected_components`) severs wings and engine nacelles while the surviving core continues flying under asymmetrical thrust yaw.

### B. 3D Falling Wreckage Lifecycle (`FallingWreckage`)
When an enemy wing or hull cleaves apart, the fragment enters `falling_wreckage` at sky altitude $z = 1.0$ with vertical velocity $v_z < 0$:
1. **Airborne Descent ($z > h(\mathbf{x})$)**:
   - Foreshortens by perspective scale $0.72 + 0.28 z$, trails smoke/sparks, and casts a shadow offset by $\Delta z = z - h(\mathbf{x})$ that converges onto the piece as it approaches the surface.
   - At high altitude ($z > 0.68$), spinning wreckage collides with and fractures other airborne enemies.
2. **Surface Touchdown ($z \le h(\mathbf{x})$) — Three Constitutive Regimes**:
   - **Tall Building Roofs ($h \in [0.68, 0.90]$)**: Strikes early at roof height, inflicts a charred structural fracture on the dome, and deflects radially down the 3D surface slope $-\nabla h(\mathbf{x})$.
   - **Solid Ground & Tarmac ($h \in [0.42, 0.54]$)**: Skids along its horizontal velocity vector to carve a tapered scorched gouge and embeds as persistent `GroundedWreckage`.
   - **Hydro-Canal Waterways ($h = 0.15$)**: Injects an acoustic impulse into the water lattice and spawns broken VGA foam spray droplets (`WaterSplash`) around a sinking hull core.

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

Running a 4B-parameter rectified-flow transformer inside a $60\text{ Hz}$ ($16.6\text{ ms}$) frame loop violates frame-time budgets, introduces temporal pixel boiling, and smears exact topological cleavage seams.

**Hybrid Pipeline (`tools/flux-forge/flux_forge.py`)**:
1. **Orthographic Mask Export (`export-masks`)**: Generates deterministic conditioning silhouettes directly from `CohesiveBody` geometry.
2. **4-Step Rectified-Flow Conditioning (`generate`)**: Runs `black-forest-labs/FLUX.2-klein-4B` (`bfloat16`) offline/at asset-build time.
3. **VGA Quantization, Seam Extraction & Heightmap Baking (`quantize`, `bake_height_and_normal_map`)**:
   - Quantizes sprites to 5-bit VGA palettes with 1px dark charcoal keylines.
   - Maps dark armor panel lines into pre-stressed local cohesion values ($C_0 = 0.84$) so hulls physically fracture along their visual seams.
   - Exports 4-channel `(nx, ny, nz, elevation_h)` maps (`*_height_normal.png`) for 2.5D lighting and roof deflection.

### Example CLI Usage (`tools/flux-forge/flux_forge.py`)

```bash
# 1. Export all built-in CohesiveBody conditioning masks, VGA sprites, height/normal maps & seam JSONs
python3 tools/flux-forge/flux_forge.py export-masks --out-dir tools/flux-forge/masks --vga-size 48

# 2. Generate all built-in enemy & building sprites on CUDA using FLUX.2 [klein] 4B (4 steps)
python3 tools/flux-forge/flux_forge.py generate \
  --model black-forest-labs/FLUX.2-klein-4B \
  --masks-dir tools/flux-forge/masks \
  --out-dir crates/shmup/assets \
  --steps 4

# 3. Generate a new custom enemy ship from a custom prompt and conditioning mask
python3 tools/flux-forge/flux_forge.py generate \
  --entity heavy_bomber \
  --mask tools/flux-forge/masks/twin_gunship_mask.png \
  --prompt "1994 MS-DOS VGA pixel art top-down heavy twin-boom stealth bomber sprite, dark slate armor, dual orange plasma bays, 1px dark keyline, solid black background" \
  --vga-size 48 \
  --out-dir crates/shmup/assets

# 4. Generate a new custom ground building (with automatic 4-channel height/normal map + cleavage seams)
python3 tools/flux-forge/flux_forge.py generate \
  --entity radar_bunker \
  --mask tools/flux-forge/masks/geodesic_dome_mask.png \
  --prompt "1994 MS-DOS VGA pixel art top-down armored concrete radar bunker building, rotating dish, steel rivets, top-left lighting, solid black background" \
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
├── entities.rs        # Sky/ground actors, FallingWreckage, GroundedWreckage, WaterSplash
├── spawning.rs        # Escalating wave formations, gunship pincers, dreadnought command waves
├── combat.rs          # Force bullets, air-to-ground missiles, progressive fracturing, 3D wreckage
├── autopilot.rs       # Autonomous combat & target-sweeping pilot
├── gui.rs             # Bevy borderless fullscreen (1080p/4K), auto-detected Gamepad, MP4 recorder
└── render/
    ├── mod.rs         # 4x sub-cell (960x540) ground/water pass, craters, horizon shadow sampling
    ├── hud.rs         # Vertical side gauges, outlined $00000000 ticker, missile ordnance badge
    ├── shadows.rs     # 2.5D Δz-converging sky drop-shadows
    ├── ships.rs       # Player interceptor & enemy squadron VGA rendering
    ├── structures.rs  # Domes, biodomes, turrets, patrol boats, canisters, and projectiles
    └── wreckage.rs    # Falling wreckage smoke trails, tapered skid gouges, VGA water splashes
```

### Verification Workflow
1. **Unit & Physics Gate**: `cargo test --workspace` verifies tri-lobe ship cleaving/annealing, progressive enemy fracturing, missile waveguide chain-detonation, and 3D falling wreckage roof/ground/water regimes.
2. **Isolated Toy Scenarios (`cargo run -p shmup -- --isolate docs`)**: Renders isolated single-feature scenes (`toy_1_player_clean.png`, `toy_2_shadows_and_roof.png`, `toy_3_wreckage_and_splash.png`) to inspect visual changes without multi-wave combat clutter.
