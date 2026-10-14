# Dzog Field Mathematics & Staggered Discretization

This document provides the governing partial differential equations (PDEs), Yee-staggered finite-difference stencils, Bohm-Hiley quantum potential formulation, and stability proofs underlying the **Dzog** engine (`dzog-core` and `dzog-wgpu`).

---

## 1. Governing Continuum Equations

Dzog evolves scalar pressure perturbation $p(\mathbf{x}, t)$ and particle velocity $\mathbf{v}(\mathbf{x}, t) = (v_x, v_y)$ across a heterogeneous 2D medium parameterized by mass density $\rho(\mathbf{x}) > 0$, bulk modulus $K(\mathbf{x}) > 0$, dissipation rate $\gamma(\mathbf{x}) \ge 0$, and structural cohesion $C(\mathbf{x}) \in [0, 1]$:

$$\rho(\mathbf{x}) \frac{\partial \mathbf{v}}{\partial t} = -\nabla p - \rho(\mathbf{x})\gamma(\mathbf{x})\mathbf{v}$$

$$\frac{\partial p}{\partial t} = -K(\mathbf{x})\nabla \cdot \mathbf{v} - \gamma(\mathbf{x})p + f_{\text{nl}}(p) + s(\mathbf{x}, t)$$

### Derived Wave Speed & Characteristic Impedance

$$c(\mathbf{x}) = \sqrt{\frac{K(\mathbf{x})}{\rho(\mathbf{x})}}, \qquad Z(\mathbf{x}) = \rho(\mathbf{x}) c(\mathbf{x}) = \sqrt{\rho(\mathbf{x}) K(\mathbf{x})}$$

When an acoustic or soliton wavefront crosses a material interface from $Z_1$ into $Z_2$, the normal reflection and transmission amplitudes follow the Fresnel relations:

$$R = \frac{Z_2 - Z_1}{Z_2 + Z_1}, \qquad T = \frac{2 Z_2}{Z_2 + Z_1}$$

---

## 2. Saturating Solitary Wave Law ($f_{\text{nl}}$)

In a purely linear medium ($f_{\text{nl}} = 0$), a localized 2D wave packet disperses radially with amplitude decaying as $r^{-1/2}$. Standard cubic Kerr nonlinearities ($\beta p^3$) counteract dispersion but suffer from finite-time blowup (`NaN`) when shockwaves constructively interfere.

Dzog introduces a rational saturating solitary flux term active in the open medium ($C(\mathbf{x}) = 0$):

$$f_{\text{nl}}(p) = \frac{\beta \, p}{1 + \alpha \, p^2} - \kappa \, p$$

- **Self-Focusing ($\beta > 0$)**: Reinforces high-amplitude wave cores so projectiles and balls persist as coherent quasi-particles (solitons).
- **Amplitude Saturation ($\alpha > 0$)**: Caps maximum nonlinear growth at $\frac{\beta}{2\sqrt{\alpha}}$, guaranteeing boundedness under arbitrary wave superposition.
- **Sub-Threshold Vacuum Scrubbing ($\kappa > 0$)**: When $\beta < \kappa + \gamma_{\text{vacuum}}$, low-amplitude dispersive tails decay exponentially while super-threshold soliton cores remain self-sustaining.

---

## 3. Bohm-Hiley Quantum Potential & Active Information ($Q$)

Following David Bohm and Basil Hiley's second-order pilot-wave formulation (*The Undivided Universe*), Dzog computes the regularized field envelope $R(\mathbf{x}, t)$ and quantum potential $Q(\mathbf{x}, t)$:

$$R(\mathbf{x}, t) = \sqrt{p(\mathbf{x}, t)^2 + \epsilon^2}, \qquad Q(\mathbf{x}, t) = -\frac{\nabla^2 R(\mathbf{x}, t)}{R(\mathbf{x}, t)} \quad (\epsilon = 0.08)$$

Unlike classical radiation pressure ($\mathbf{F}_p = -\nabla p$), which scales with raw amplitude, $Q$ is normalized by $R(\mathbf{x}, t)$ and therefore measures the **spatial curvature (form)** of the wavefield:
- Constructive interference ridges and standing-wave channels create negative $Q$ valleys ($\nabla^2 R > 0 \implies Q < 0$).
- Field-coupled actors experience the combined guidance law:
  $$\mathbf{F}_{\text{total}} = -\kappa_p \nabla p - \kappa_Q \nabla Q - m \nabla \Phi_g$$

---

## 4. Long-Range Gravitational Potential ($\Phi_g$)

For orbital mechanics and gravity-well arenas (`crates/dzog-core/src/gravity.rs`), a scalar gravitational potential $\Phi_g(\mathbf{x})$ satisfies the screened Poisson equation:

$$\nabla^2 \Phi_g(\mathbf{x}) = 4\pi G \left(\rho(\mathbf{x}) - \rho_0\right) + \sum_{k} S_k(\mathbf{x})$$

relaxed iteratively via damped Jacobi / Red-Black Gauss-Seidel updates on the lattice alongside softened analytical singularity wells:

$$\mathbf{g}(\mathbf{x}) = -\nabla \Phi_g(\mathbf{x}) + \sum_{k} \frac{M_k (\mathbf{x}_k - \mathbf{x})}{\left(|\mathbf{x}_k - \mathbf{x}|^2 + r_{0,k}^2\right)^{3/2}}$$

---

## 5. Yee-Staggered Finite-Difference Discretization

To eliminate odd-even pressure checkerboarding and ensure exact discrete flux conservation across solid boundaries, variables are staggered in both space and time:

- **Cell Centers $(i + \frac{1}{2}, j + \frac{1}{2})$ at integer steps $t^n$**:
  $p_{i,j}^n, \; Q_{i,j}^n, \; \rho_{i,j}, \; K_{i,j}, \; \gamma_{i,j}, \; C_{i,j}$
- **East Faces $(i + 1, j + \frac{1}{2})$ at half steps $t^{n + 1/2}$**:
  $v_{x; i,j}^{n + 1/2}$
- **North Faces $(i + \frac{1}{2}, j + 1)$ at half steps $t^{n + 1/2}$**:
  $v_{y; i,j}^{n + 1/2}$

### Pass 1: Staggered Velocity Update (`velocity.wgsl`)

With face-averaged density $\rho_{e; i,j} = \frac{1}{2}(\rho_{i,j} + \rho_{i+1,j})$ and damping $\gamma_{e; i,j} = \frac{1}{2}(\gamma_{i,j} + \gamma_{i+1,j})$:

$$v_{x; i,j}^{n+1/2} = \begin{cases} v_{\text{wall}, x; i,j} & \text{if } \text{wall\_east}_{i,j} = \text{true} \\[6pt] e^{-\gamma_{e; i,j} \Delta t} \, v_{x; i,j}^{n-1/2} - \dfrac{\Delta t}{\rho_{e; i,j}} \dfrac{p_{i+1,j}^n - p_{i,j}^n}{\Delta x} & \text{otherwise} \end{cases}$$

(and analogously for $v_{y; i,j}^{n+1/2}$ across North faces).

### Pass 2: Staggered Pressure & Cohesion Update (`pressure.wgsl`)

$$\left(\nabla \cdot \mathbf{v}\right)_{i,j}^{n+1/2} = \frac{v_{x; i,j}^{n+1/2} - v_{x; i-1,j}^{n+1/2}}{\Delta x} + \frac{v_{y; i,j}^{n+1/2} - v_{y; i,j-1}^{n+1/2}}{\Delta y}$$

$$p_{\text{lin}; i,j}^{n+1} = e^{-\gamma_{i,j} \Delta t} \, p_{i,j}^n - K_{i,j} \Delta t \left(\nabla \cdot \mathbf{v}\right)_{i,j}^{n+1/2}$$

$$p_{i,j}^{n+1} = p_{\text{lin}; i,j}^{n+1} + \Delta t \, f_{\text{nl}}\!\left(p_{\text{lin}; i,j}^{n+1}\right)$$

When $|p_{i,j}^{n+1}| > p_{\text{yield}; i,j}$, local structural cohesion erodes continuously:

$$C_{i,j}^{n+1} = \max\!\left(0, \; C_{i,j}^n - \eta_{\text{erode}} \left(|p_{i,j}^{n+1}| - p_{\text{yield}; i,j}\right) \Delta t\right)$$

### Pass 3: Screened Reaction-Diffusion Luminescence Update (`luminescence.wgsl`)

Field-driven radiative glow $L_{i,j}^n \in [0, 4]$ evolves on the lattice via material-tuned diffusion $D_{i,j} \propto c_{i,j} = \sqrt{K_{i,j}/\rho_{i,j}}$ (enhanced along semi-permeable crystal waveguides $0.15 \le C_{i,j} < 0.55$), exponential radiative decay $e^{-\gamma_L \Delta t}$, and wave/piezoelectric source coupling:

$$L_{i,j}^{n+1} = \text{clamp}\!\left(e^{-\gamma_L \Delta t}\left(L_{i,j}^n + D_{i,j} \Delta t \, \nabla^2 L_{i,j}^n\right) + \Delta t \left(S_{\text{wave}}(p, \mathbf{v}) + S_{\text{piezo}}(p, C)\right), \; 0, \; 4\right)$$

Spatial gradients $\nabla L_{i,j}$ dynamically illuminate nearby 3D mineralogical crystal facets (`CohesiveBody::sample_crystal_facet`).

---

## 6. CFL Stability Envelope & Hamiltonian Energy

### Global Courant-Friedrichs-Lewy (CFL) Bound

Before stepping, `SolverConfig::compute_cfl_dt` locks $\Delta t$ against the maximum possible wave speed across the material field:

$$c_{\text{bound}} = \sqrt{\frac{\max_{i,j} K_{i,j}}{\min_{i,j} \rho_{i,j}}}, \qquad \Delta t \le \frac{\text{safety}}{c_{\text{bound}} \sqrt{\Delta x^{-2} + \Delta y^{-2}}}, \qquad 0 < \text{safety} < 1$$

### Hamiltonian Energy & Acoustic Poynting Flux

Total mechanical energy $H = E_p + E_k$ and instantaneous energy flux $\mathbf{I}$ are integrated over cell area $\Delta A = \Delta x \Delta y$:

$$E_p = \sum_{i,j} \frac{(p_{i,j})^2}{2 K_{i,j}} \Delta A, \qquad E_k = \sum_{i,j} \frac{1}{2} \rho_{i,j} \left((v_{x; i,j})^2 + (v_{y; i,j})^2\right) \Delta A, \qquad \mathbf{I}_{i,j} = p_{i,j} \mathbf{v}_{i,j}$$
