use egui::PaintCallbackInfo;
use egui_wgpu::{CallbackResources, CallbackTrait};
use wgpu::RenderPass;

pub trait Paintable {
    fn paint<'a>(&self, render_pass: &mut RenderPass<'a>);
}

// // A callback that contains a Paintable object
// pub struct PaintableCallback<T: Paintable + Send + Sync + 'static> {
//     paintable: T,
// }

// impl<T: Paintable + Send + Sync + 'static> PaintableCallback<T> {
//     pub fn new(paintable: T) -> Self {
//         Self { paintable }
//     }
// }

// impl<T: Paintable + Send + Sync + 'static> egui_wgpu::CallbackTrait for PaintableCallback<T> {
//     fn paint(
//         &self,
//         _info: PaintCallbackInfo,
//         render_pass: &mut wgpu::RenderPass<'static>,
//         _callback_resources: &CallbackResources,
//     ) {
//         // This ugly transmute is needed because the CallbackTrait demands 'static
//         // for the render_pass, but we need a shorter lifetime to call our paintable
//         // Safe because the render_pass is guaranteed to outlive the paint call
//         unsafe {
//             let render_pass_with_shorter_lifetime = std::mem::transmute::<
//                 &mut wgpu::RenderPass<'static>,
//                 &mut wgpu::RenderPass<'_>,
//             >(render_pass);
//             self.paintable.paint(render_pass_with_shorter_lifetime);
//         }
//     }
// }

pub struct ParticlePainter {
    pub render_pipeline: wgpu::RenderPipeline,
    pub camera_bind_group: wgpu::BindGroup,
    pub particle_buffer: wgpu::Buffer,
    pub num_particles: u32,
}

impl Paintable for ParticlePainter {
    fn paint<'a>(&self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.particle_buffer.slice(..));
        render_pass.draw(0..1, 0..self.num_particles);
    }
}

pub struct UnsafeParticleCallback {
    pub render_pipeline_ptr: *const wgpu::RenderPipeline,
    pub camera_bind_group_ptr: *const wgpu::BindGroup,
    pub particle_buffer_ptr: *const wgpu::Buffer,
    pub num_particles: u32,
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

            render_pass.set_pipeline(render_pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, particle_buffer.slice(..));
            render_pass.draw(0..1, 0..self.num_particles);
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
