use egui::PaintCallbackInfo;
use egui_wgpu::{CallbackResources, CallbackTrait};

use crate::simulation::Particle;

pub struct UnsafeParticleCallback {
    pub render_pipeline_ptr: *const wgpu::RenderPipeline,
    pub camera_bind_group_ptr: *const wgpu::BindGroup,
    pub particle_buffer_ptr: *const wgpu::Buffer,
}

// Safe because we ensure the pointers remain valid during the callback's lifetime
// and we're only reading from them
unsafe impl Send for UnsafeParticleCallback {}
unsafe impl Sync for UnsafeParticleCallback {}

impl CallbackTrait for UnsafeParticleCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        Vec::new()
    }

    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &CallbackResources,
    ) {
        unsafe {
            // Dereference the pointers - safe because we ensure they remain valid
            let render_pipeline = &*self.render_pipeline_ptr;
            let camera_bind_group = &*self.camera_bind_group_ptr;
            let particle_buffer = &*self.particle_buffer_ptr;

            let particle_struct_size = std::mem::size_of::<Particle>() as u64;
            // Ensure particle_struct_size is not zero to avoid division by zero
            let actual_drawable_particles = if particle_struct_size > 0 {
                particle_buffer.size() / particle_struct_size
            } else {
                0 // Or handle error appropriately
            };

            render_pass.set_pipeline(render_pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, particle_buffer.slice(..));
            render_pass.draw(0..1, 0..actual_drawable_particles as u32);
        }
    }
}

// A callback for particle rendering using cloned resources
pub struct ClonedParticleCallback {
    pub render_pipeline: wgpu::RenderPipeline,
    pub camera_bind_group: wgpu::BindGroup,
    pub particle_buffer: wgpu::Buffer,
    pub num_particles: u32,
}

impl CallbackTrait for ClonedParticleCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        Vec::new()
    }

    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &CallbackResources,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.particle_buffer.slice(..));
        render_pass.draw(0..1, 0..self.num_particles);
    }
}
