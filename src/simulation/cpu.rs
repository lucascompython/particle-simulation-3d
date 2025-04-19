use super::{Particle, generate_initial_particles};
use super::{ParticleSimulation, SimParams, SimulationMethod};
use glam::Vec3;
use wgpu::util::DeviceExt;

pub struct CpuParticleSimulation {
    particles: Vec<Particle>,
    particle_buffer: wgpu::Buffer,
    particle_count: u32,
    paused: bool,
}

impl ParticleSimulation for CpuParticleSimulation {
    fn new(device: &wgpu::Device, initial_particle_count: u32) -> Self {
        let particles = generate_initial_particles(initial_particle_count);

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
        }
    }

    fn update(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        params: &SimParams,
    ) {
        if self.paused {
            return;
        }
        // Update particles on CPU
        for particle in &mut self.particles[0..self.particle_count as usize] {
            // Apply gravity
            let mut velocity = Vec3::from(particle.velocity);
            velocity.y -= params.gravity * params.delta_time;

            // Apply mouse force
            if params.is_mouse_dragging > 0 {
                let position = Vec3::from(particle.position);
                let mouse_pos = Vec3::from(params.mouse_position);
                let dir = mouse_pos - position;
                let dist = dir.length();

                if dist < params.mouse_radius * 2.0 {
                    let force_factor = (1.0 - dist / (params.mouse_radius * 2.0)).powi(2) * 2.0;
                    let force = dir.normalize() * params.mouse_force * force_factor;
                    velocity += force * params.delta_time;
                }
            }

            // Update position
            let mut position = Vec3::from(particle.position);
            position += velocity * params.delta_time;

            // Boundary checks
            let bounds = 500.0;
            if position.x < -bounds {
                position.x = -bounds;
                velocity.x = velocity.x.abs() * 0.5;
            } else if position.x > bounds {
                position.x = bounds;
                velocity.x = -velocity.x.abs() * 0.5;
            }

            if position.y < -bounds {
                position.y = -bounds;
                velocity.y = velocity.y.abs() * 0.5;
            } else if position.y > bounds {
                position.y = bounds;
                velocity.y = -velocity.y.abs() * 0.5;
            }

            if position.z < -bounds {
                position.z = -bounds;
                velocity.z = velocity.z.abs() * 0.5;
            } else if position.z > bounds {
                position.z = bounds;
                velocity.z = -velocity.z.abs() * 0.5;
            }

            // Apply damping
            velocity *= params.damping;

            // Update color based on mode
            let color = match params.color_mode {
                1 => {
                    // Velocity-based
                    let speed = velocity.length();
                    let norm_speed = (speed / 5.0).min(1.0);
                    [norm_speed, 0.5 - norm_speed * 0.5, 1.0 - norm_speed, 1.0]
                }
                2 => {
                    // Position-based
                    let norm_pos = (position / bounds + Vec3::ONE) * 0.5;
                    [norm_pos.x, norm_pos.y, norm_pos.z, 1.0]
                }
                _ => particle.color, // Keep original
            };

            // Update the particle
            particle.position = position.into();
            particle.velocity = velocity.into();
            particle.color = color;
        }

        // Upload updated data to GPU
        queue.write_buffer(
            &self.particle_buffer,
            0,
            bytemuck::cast_slice(&self.particles[0..self.particle_count as usize]),
        );
    }

    fn resize_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, new_count: u32) {
        if new_count > self.particles.len() as u32 {
            // Expand the particle vector
            let additional_count = new_count - self.particles.len() as u32;
            let mut new_particles = generate_initial_particles(additional_count);
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

    fn reset(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.particles = generate_initial_particles(self.particle_count);

        // If buffer size might have changed, recreate it
        self.particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CPU Particle Buffer"),
            contents: bytemuck::cast_slice(&self.particles),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
        });

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
