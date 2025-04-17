mod compute_particles;
mod fragment_particles;

use wgpu::CommandEncoder;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Particle {
    pub position: [f32; 3],
    pub padding1: f32,
    pub velocity: [f32; 3],
    pub padding2: f32,
    pub color: [f32; 4],
}

pub trait ParticleSimulator {
    fn update(&mut self, queue: &wgpu::Queue, encoder: &mut CommandEncoder);
    fn reset(&mut self, queue: &wgpu::Queue);
    fn get_particle_buffer(&self) -> &wgpu::Buffer;
    fn get_vertex_buffer(&self) -> &wgpu::Buffer;
    fn get_max_particles(&self) -> u32;
    fn get_num_particles(&self) -> u32;
    fn set_num_particles(&mut self, num: u32);
    fn get_gravity(&self) -> f32;
    fn set_gravity(&mut self, gravity: f32);
    fn get_color_mode(&self) -> u32;
    fn set_color_mode(&mut self, mode: u32);
    fn get_mouse_position(&self) -> [f32; 3];
    fn set_mouse_position(&mut self, pos: [f32; 3]);
    fn get_mouse_force(&self) -> f32;
    fn set_mouse_force(&mut self, force: f32);
    fn get_mouse_radius(&self) -> f32;
    fn set_mouse_radius(&mut self, radius: f32);
    fn is_paused(&self) -> bool;
    fn set_paused(&mut self, paused: bool);
    fn set_mouse_dragging(&mut self, dragging: bool);
    fn is_mouse_dragging(&self) -> bool;
    fn get_particle_size(&self) -> f32;
    fn set_particle_size(&mut self, size: f32);
}

// Export both implementations
pub use compute_particles::ComputeParticleSystem;
pub use fragment_particles::FragmentParticleSystem;

// Function to create appropriate particle system based on capabilities
pub fn create_particle_system(
    device: &wgpu::Device,
    compute_shader: Option<&wgpu::ShaderModule>,
    fragment_shader: &wgpu::ShaderModule,
    is_web: bool,
) -> Box<dyn ParticleSimulator> {
    // Default particle counts
    let default_particles = if is_web { 100_000 } else { 1_000_000 };

    // Check if compute shaders with storage buffers are supported
    let compute_supported =
        compute_shader.is_some() && device.limits().max_storage_buffers_per_shader_stage > 0;

    if compute_supported {
        // Use compute shader implementation
        log::info!("Using compute shader particle system");
        Box::new(ComputeParticleSystem::new(
            device,
            default_particles,
            compute_shader.unwrap(),
        ))
    } else {
        // Use fragment shader fallback
        log::info!("Using fragment shader particle system fallback");
        Box::new(FragmentParticleSystem::new(
            device,
            default_particles,
            fragment_shader,
        ))
    }
}
