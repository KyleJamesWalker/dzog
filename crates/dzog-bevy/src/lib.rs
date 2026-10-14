//! Bevy integration plugin for the Dzog field-physics & rendering engine.
//!
//! Provides shared Bevy rendering, WGPU compute acceleration, perceptual view modes,
//! and native `bevy_audio` (`Decodable` / `AudioPlayer`) synthesis so games (`asteroids`, `pong`)
//! do not duplicate host engine glue.

use std::num::NonZero;
use std::sync::{Arc, Mutex};
use bevy::{
    asset::RenderAssetUsages,
    audio::{AddAudioSource, AudioPlayer, Decodable, PlaybackSettings, Source},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use dzog_core::{
    AcousticSolver, AcousticState, BoundaryMask, FieldAcousticFrame, FieldModalSynthesizer,
    MaterialField, OuterBoundaryRule, SolverConfig,
};
use dzog_wgpu::{GpuBodyQuery, GpuBodyReduction};
#[cfg(not(target_arch = "wasm32"))]
use dzog_wgpu::{GpuContext, GpuSimulation};
#[cfg(target_arch = "wasm32")]
use dzog_wgpu::reduce_bodies_cpu;

/// Bevy plugin registering Dzog's procedural field-audio asset (`DzogFieldAudioSource`).
pub struct DzogBevyPlugin;

impl Plugin for DzogBevyPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<DzogFieldAudioSource>();
    }
}

/// Field compute backend: the WGPU pipeline natively, the `dzog-core` reference solver on wasm32,
/// where the per-frame blocking GPU readback is unavailable.
pub struct GpuFieldAccelerator {
    #[cfg(not(target_arch = "wasm32"))]
    pub ctx: GpuContext,
    #[cfg(not(target_arch = "wasm32"))]
    pub sim: GpuSimulation,
}

#[cfg(not(target_arch = "wasm32"))]
impl GpuFieldAccelerator {
    pub fn new(
        state: &AcousticState,
        material: &MaterialField,
        boundary: &BoundaryMask,
        config: &SolverConfig,
    ) -> Self {
        let ctx = GpuContext::shared().clone();
        let sim = GpuSimulation::new(&ctx, state, material, boundary, config)
            .expect("Failed to initialize WGPU simulation pipeline");
        Self { ctx, sim }
    }

    pub fn set_outer_rule(&mut self, outer_rule: OuterBoundaryRule) {
        self.sim.set_outer_rule(&self.ctx, outer_rule);
    }

    /// Advance `substeps` FDTD substeps of `solver`, including its step clock.
    pub fn step(&self, solver: &mut AcousticSolver, substeps: usize) {
        self.sim.upload_state(&self.ctx, &solver.state);
        self.sim.upload_material(&self.ctx, &solver.material);
        self.sim.upload_boundary(&self.ctx, &solver.boundary);
        self.sim.step(&self.ctx, substeps);
        self.sim.readback_state_into(&self.ctx, &mut solver.state);
        advance_clock(solver, substeps);
    }

    /// Advance like [`Self::step`], then reduce body forces and probes for `queries`.
    pub fn step_and_reduce(
        &self,
        solver: &mut AcousticSolver,
        substeps: usize,
        queries: &[GpuBodyQuery],
    ) -> Vec<GpuBodyReduction> {
        let reductions = self.sim.step_and_reduce_into(
            &self.ctx,
            substeps,
            &mut solver.state,
            &solver.material,
            &solver.boundary,
            queries,
        );
        advance_clock(solver, substeps);
        reductions
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn advance_clock(solver: &mut AcousticSolver, substeps: usize) {
    solver.current_step += substeps;
    solver.current_time += solver.config.dt * substeps as f32;
}

#[cfg(target_arch = "wasm32")]
impl GpuFieldAccelerator {
    pub fn new(
        _state: &AcousticState,
        _material: &MaterialField,
        _boundary: &BoundaryMask,
        _config: &SolverConfig,
    ) -> Self {
        Self {}
    }

    pub fn set_outer_rule(&mut self, _outer_rule: OuterBoundaryRule) {}

    /// Advance `substeps` FDTD substeps of `solver`, including its step clock.
    pub fn step(&self, solver: &mut AcousticSolver, substeps: usize) {
        for _ in 0..substeps {
            solver.step();
        }
    }

    /// Advance like [`Self::step`], then reduce body forces and probes for `queries`.
    pub fn step_and_reduce(
        &self,
        solver: &mut AcousticSolver,
        substeps: usize,
        queries: &[GpuBodyQuery],
    ) -> Vec<GpuBodyReduction> {
        self.step(solver, substeps.saturating_sub(1));
        let q_pressure = solver.state.pressure.clone();
        self.step(solver, substeps.min(1));
        let periodic = solver.boundary.outer_rule == OuterBoundaryRule::Periodic;
        reduce_bodies_cpu(&solver.state, &q_pressure, periodic, queries)
    }
}

/// Perceptual inspection view mode (`philosophy.md` §Multiple Perceptual Views).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldViewMode {
    /// Default arcade view: crisp vacuum, solid-state crystal cleavage veins, and shockwave rings.
    #[default]
    CompositeArcade,
    /// Scalar pressure p(x,y) and Bohmian Quantum Potential Q(x,y) guidance ridges.
    PressureAndBohmian,
    /// Particle velocity |v|, Poynting energy flux p*v, and gravitational potential Phi_g.
    VelocityAndFlux,
    /// Material acoustic impedance Z(x,y) = sqrt(rho * K) and structural cohesion C(x,y).
    ImpedanceAndCohesion,
}

impl FieldViewMode {
    pub fn next(self) -> Self {
        match self {
            Self::CompositeArcade => Self::PressureAndBohmian,
            Self::PressureAndBohmian => Self::VelocityAndFlux,
            Self::VelocityAndFlux => Self::ImpedanceAndCohesion,
            Self::ImpedanceAndCohesion => Self::CompositeArcade,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CompositeArcade => "Composite Ecology",
            Self::PressureAndBohmian => "Pressure p & Bohmian Q",
            Self::VelocityAndFlux => "Velocity v & Energy Flux",
            Self::ImpedanceAndCohesion => "Impedance Z & Cohesion C",
        }
    }
}

/// Bevy procedural audio decoder wrapping `dzog_core::FieldModalSynthesizer`.
pub struct DzogFieldDecoder(pub FieldModalSynthesizer);

impl Iterator for DzogFieldDecoder {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl Source for DzogFieldDecoder {
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> NonZero<u16> {
        NonZero::new(2).unwrap()
    }

    #[inline]
    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(FieldModalSynthesizer::DEFAULT_SAMPLE_RATE).unwrap()
    }

    #[inline]
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Bevy `Asset` + `Decodable` audio source streaming live PDE lattice sonification via `bevy_audio`.
#[derive(Asset, TypePath, Clone)]
pub struct DzogFieldAudioSource {
    pub shared_frame: Arc<Mutex<FieldAcousticFrame>>,
}

impl Decodable for DzogFieldAudioSource {
    type Decoder = DzogFieldDecoder;

    fn decoder(&self) -> Self::Decoder {
        DzogFieldDecoder(FieldModalSynthesizer::new(self.shared_frame.clone()))
    }
}

/// Bevy resource holding the handle to the dynamic field texture canvas.
#[derive(Resource, Clone)]
pub struct FieldImageHandle(pub Handle<Image>);

/// Spawn the 2D nearest-neighbor field texture sprite and optional `AudioPlayer` in a Bevy scene.
pub fn spawn_field_canvas_and_audio(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    audio_assets: &mut Assets<DzogFieldAudioSource>,
    width: u32,
    height: u32,
    initial_pixels: &[u8],
    display_size: Vec2,
    y_offset: f32,
    shared_audio: Option<Arc<Mutex<FieldAcousticFrame>>>,
) {
    commands.spawn(Camera2d);

    if let Some(shared_frame) = shared_audio {
        let source_handle = audio_assets.add(DzogFieldAudioSource { shared_frame });
        commands.spawn((AudioPlayer(source_handle), PlaybackSettings::LOOP));
    }

    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        initial_pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();

    let image_handle = images.add(image);
    commands.insert_resource(FieldImageHandle(image_handle.clone()));

    commands.spawn((
        Sprite {
            image: image_handle,
            custom_size: Some(display_size),
            ..default()
        },
        Transform::from_xyz(0.0, y_offset, 0.0),
    ));
}
