use super::{Particle, SphereGeneration, generate_initial_particles};
use super::{ParticleSimulation, SimParams, SimulationMethod};
use glam::Vec3;
use rayon::prelude::*;
use wgpu::util::DeviceExt;

pub struct CpuParticleSimulation {
    particles: Vec<Particle>,
    particle_buffer: wgpu::Buffer,
    particle_count: u32,
    paused: bool,
    generation_mode: SphereGeneration,
}

impl ParticleSimulation for CpuParticleSimulation {
    fn new(
        device: &wgpu::Device,
        initial_particle_count: u32,
        _surface_format: wgpu::TextureFormat,
        generation_mode: SphereGeneration,
    ) -> Self {
        let particles = generate_initial_particles(initial_particle_count, generation_mode);

        let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CPU Particle Buffer"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
        });

        Self {
            particles,
            particle_buffer,
            particle_count: initial_particle_count,
            paused: false,
            generation_mode,
        }
    }

    fn update(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        params: &SimParams,
    ) {
        // if self.paused {
        //     return;
        // }

        // Create local references to simulation parameters for better cache locality
        let delta_time = params.delta_time;
        let gravity = params.gravity;
        let mouse_force = params.mouse_force;
        let mouse_radius = params.mouse_radius;
        let mouse_dragging = params.is_mouse_dragging > 0;
        let damping = params.damping;
        let color_mode = params.color_mode;
        let mouse_pos = Vec3::from(params.mouse_position);
        let max_dist = params.max_dist_for_color;

        // Use Rayon to parallelize particle updates
        // Only process up to particle_count
        let active_particles = &mut self.particles[0..self.particle_count as usize];

        active_particles.par_iter_mut().for_each(|particle| {
            // Extract position and velocity once to minimize conversions
            let mut position = Vec3::from(particle.position);
            let mut velocity = Vec3::from(particle.velocity);
            let initial_color = particle.initial_color;

            // Apply gravity
            velocity.y -= gravity * delta_time;

            // Apply mouse force - only calculate if dragging
            if mouse_dragging {
                let dir = mouse_pos - position;
                let dist = dir.length();

                if dist < mouse_radius * 2.0 {
                    let force_factor = (1.0 - dist / (mouse_radius * 2.0)).powi(2) * 2.0;
                    let force = dir.normalize() * mouse_force * force_factor;
                    velocity += force * delta_time;
                }
            }

            // Update position
            position += velocity * delta_time;

            // Apply damping
            velocity *= damping;

            // Update color based on mode - using match for better performance
            let color = match color_mode {
                1 => {
                    // Velocity-based
                    let speed = velocity.length();
                    let norm_speed = (speed / 5.0).min(1.0);
                    [norm_speed, 0.5 - norm_speed * 0.5, 1.0 - norm_speed, 1.0]
                }
                2 => {
                    // Position-based (distance from origin)
                    let dist_from_origin = position.length();
                    let norm_dist = (dist_from_origin / max_dist.max(0.01)).clamp(0.0, 1.0);
                    [norm_dist, 0.0, 1.0 - norm_dist, 1.0] // Blue near, Red far
                }
                _ => particle.color, // Keep original
            };

            // Update the particle
            particle.position = position.into();
            particle.velocity = velocity.into();
            particle.color = color;
        });

        // Upload updated data to GPU
        queue.write_buffer(
            &self.particle_buffer,
            0,
            bytemuck::cast_slice(&self.particles[0..self.particle_count as usize]),
        );
    }

    fn resize_buffer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        new_count: u32,
        generation_mode: SphereGeneration,
    ) {
        self.generation_mode = generation_mode;

        if new_count == self.particle_count {
            return;
        }

        if new_count > self.particles.len() as u32 {
            // Expand the particle vector
            let additional_count = new_count - self.particles.len() as u32;
            let mut new_particles = generate_initial_particles(additional_count, generation_mode);
            self.particles.append(&mut new_particles);

            // Create a new buffer with larger size
            self.particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("CPU Particle Buffer"),
                contents: bytemuck::cast_slice(&self.particles),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
            });
        }

        self.particle_count = new_count;

        // Upload current data to buffer
        queue.write_buffer(
            &self.particle_buffer,
            0,
            bytemuck::cast_slice(&self.particles[0..self.particle_count as usize]),
        );
    }

    fn get_particle_buffer(&self) -> &wgpu::Buffer {
        &self.particle_buffer
    }

    fn get_method(&self) -> SimulationMethod {
        SimulationMethod::Cpu
    }

    fn get_particle_count(&self) -> u32 {
        self.particle_count
    }

    fn reset(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        generation_mode: SphereGeneration,
    ) {
        self.generation_mode = generation_mode;
        self.particles = generate_initial_particles(self.particle_count, generation_mode);

        queue.write_buffer(
            &self.particle_buffer,
            0,
            bytemuck::cast_slice(&self.particles[0..self.particle_count as usize]),
        );
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
}
