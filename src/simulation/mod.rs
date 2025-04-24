use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};
use rand::{Rng, SeedableRng};
use wgpu::{CommandEncoder, Device, Queue};

pub mod compute;
pub mod cpu;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimulationMethod {
    Cpu,
    ComputeShader,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SphereGeneration {
    Hollow,
    Filled,
}

pub trait ParticleSimulation {
    fn new(
        device: &Device,
        initial_particle_count: u32,
        surface_format: wgpu::TextureFormat,
        generation_mode: SphereGeneration,
    ) -> Self
    where
        Self: Sized;
    fn update(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        params: &SimParams,
    );
    fn resize_buffer(
        &mut self,
        device: &Device,
        queue: &Queue,
        new_count: u32,
        generation_mode: SphereGeneration,
    );
    fn get_particle_buffer(&self) -> &wgpu::Buffer;
    fn get_method(&self) -> SimulationMethod;
    fn get_particle_count(&self) -> u32;
    fn reset(&mut self, device: &Device, queue: &Queue, generation_mode: SphereGeneration);
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
    pub max_dist_for_color: f32,

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
            max_dist_for_color: 50.0,
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

    pub initial_color: [f32; 4],
}

impl Particle {
    fn new(position: Vec3, velocity: Vec3, initial_color: Vec4) -> Self {
        Self {
            position: position.into(),
            padding1: 0.0,
            velocity: velocity.into(),
            padding2: 0.0,
            color: initial_color.into(),
            initial_color: initial_color.into(),
        }
    }
}

// pub fn generate_initial_particles(count: u32, mode:) -> Vec<Particle> {
//     if count == 0 {
//         return Vec::new();
//     }

//     let mut particles = Vec::with_capacity(count as usize);
//     let sphere_radius = 50.0; // Initial radius of the sphere
//     let golden_angle = std::f32::consts::PI * (3.0 - (5.0_f32).sqrt());

//     for i in 0..count {
//         let y = 1.0 - (i as f32 / (count - 1) as f32) * 2.0; // y goes from 1 to -1
//         let radius_at_y = (1.0 - y * y).sqrt(); // radius at y
//         let theta = golden_angle * i as f32; // golden angle increment

//         let x = theta.cos() * radius_at_y;
//         let z = theta.sin() * radius_at_y;

//         let pos = Vec3::new(x, y, z) * sphere_radius;

//         // Initial velocity (optional, could be Vec3::ZERO)
//         // let vel = pos.normalize() * 0.1; // Small outward velocity
//         let vel = Vec3::ZERO;

//         // Initial color (e.g., based on position or just white)
//         let norm_pos = (pos / sphere_radius + Vec3::ONE) * 0.5;
//         let color = Vec4::new(norm_pos.x, norm_pos.y, norm_pos.z, 1.0);

//         particles.push(Particle::new(pos, vel, color));
//     }

//     particles
// }
pub fn generate_initial_particles(count: u32, mode: SphereGeneration) -> Vec<Particle> {
    let mut particles = Vec::with_capacity(count as usize);
    let sphere_radius = 50.0; // Initial radius of the sphere

    match mode {
        SphereGeneration::Hollow => {
            let golden_angle = std::f32::consts::PI * (3.0 - (5.0_f32).sqrt());
            for i in 0..count {
                let y = 1.0 - (i as f32 / (count.max(1) - 1) as f32) * 2.0; // y goes from 1 to -1
                let radius_at_y = (1.0 - y * y).sqrt(); // radius at y
                let theta = golden_angle * i as f32; // golden angle increment

                let x = theta.cos() * radius_at_y;
                let z = theta.sin() * radius_at_y;

                let pos = Vec3::new(x, y, z) * sphere_radius;
                let vel = Vec3::ZERO;
                let norm_pos = (pos / sphere_radius + Vec3::ONE) * 0.5;
                let initial_color = Vec4::new(norm_pos.x, norm_pos.y, norm_pos.z, 1.0);

                particles.push(Particle::new(pos, vel, initial_color));
            }
        }
        SphereGeneration::Filled => {
            // Use RNG for filled sphere
            let mut rng = rand::rngs::SmallRng::seed_from_u64(69); // Use a fixed seed for reproducibility
            for _ in 0..count {
                // Uniform distribution within a sphere volume
                let r = sphere_radius * rng.random::<f32>().cbrt(); // Cube root for uniform volume
                let theta = rng.random::<f32>() * 2.0 * std::f32::consts::PI;
                let phi = (rng.random::<f32>() * 2.0 - 1.0).acos(); // Uniform spherical coordinates

                let x = r * phi.sin() * theta.cos();
                let y = r * phi.cos();
                let z = r * phi.sin() * theta.sin();

                let pos = Vec3::new(x, y, z);
                let vel = Vec3::ZERO;
                let norm_pos = (pos / sphere_radius + Vec3::ONE) * 0.5; // Color based on normalized position
                let initial_color = Vec4::new(norm_pos.x, norm_pos.y, norm_pos.z, 1.0);

                particles.push(Particle::new(pos, vel, initial_color));
            }
        }
    }

    particles
}
