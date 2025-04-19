use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};
use wgpu::{CommandEncoder, Device, Queue};

pub mod compute;
pub mod cpu;
pub mod transform_feedback;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimulationMethod {
    Cpu,
    ComputeShader,
    TransformFeedback,
}

pub trait ParticleSimulation {
    fn new(device: &Device, initial_particle_count: u32) -> Self
    where
        Self: Sized;
    fn update(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        params: &SimParams,
    );
    fn resize_buffer(&mut self, device: &Device, queue: &Queue, new_count: u32);
    fn get_particle_buffer(&self) -> &wgpu::Buffer;
    fn get_method(&self) -> SimulationMethod;
    fn get_particle_count(&self) -> u32;
    fn reset(&mut self, device: &Device, queue: &Queue);
    fn is_paused(&self) -> bool;
    fn set_paused(&mut self, paused: bool);
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimParams {
    pub delta_time: f32,
    pub gravity: f32,
    pub color_mode: u32,
    pub mouse_force: f32,

    pub mouse_radius: f32,
    pub is_mouse_dragging: u32,
    pub damping: f32,
    pub _padding1: u32,

    pub mouse_position: [f32; 3],
    pub _padding2: u32,
}

impl Default for SimParams {
    fn default() -> Self {
        Self {
            delta_time: 0.016,
            gravity: 0.0,
            color_mode: 0,
            mouse_force: 5.0,
            mouse_radius: 10.0,
            is_mouse_dragging: 0,
            damping: 0.99,
            _padding1: 0,
            mouse_position: [0.0, 0.0, 0.0],
            _padding2: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Particle {
    pub position: [f32; 3],
    pub padding1: f32,
    pub velocity: [f32; 3],
    pub padding2: f32,
    pub color: [f32; 4],
}

impl Particle {
    fn new(position: Vec3, velocity: Vec3, color: Vec4) -> Self {
        Self {
            position: position.into(),
            padding1: 0.0,
            velocity: velocity.into(),
            padding2: 0.0,
            color: color.into(),
        }
    }
}

pub fn generate_initial_particles(count: u32) -> Vec<Particle> {
    use rand::{Rng, SeedableRng, rngs::SmallRng};

    let mut rng = SmallRng::seed_from_u64(69);
    let mut particles = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let r = rng.random::<f32>();
        let g = rng.random::<f32>();
        let b = rng.random::<f32>();

        let phi = rng.random::<f32>() * std::f32::consts::PI * 2.0;
        let theta = (rng.random::<f32>() - 0.5) * std::f32::consts::PI;
        let radius = rng.random::<f32>() * 50.0;

        let pos = Vec3::new(
            radius * phi.cos() * theta.cos(),
            radius * theta.sin(),
            radius * phi.sin() * theta.cos(),
        );

        let vel = Vec3::new(
            (rng.random::<f32>() - 0.5) * 0.1,
            (rng.random::<f32>() - 0.5) * 0.1,
            (rng.random::<f32>() - 0.5) * 0.1,
        );

        particles.push(Particle::new(pos, vel, Vec4::new(r, g, b, 1.0)));
    }

    particles
}
