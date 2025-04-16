use egui::PaintCallbackInfo;
use egui_wgpu::CallbackResources;
use wgpu::RenderPass;

pub trait Paintable {
    fn paint<'a>(&self, render_pass: &mut RenderPass<'a>);
}

// A callback that contains a Paintable object
pub struct PaintableCallback<T: Paintable + Send + Sync + 'static> {
    paintable: T,
}

impl<T: Paintable + Send + Sync + 'static> PaintableCallback<T> {
    pub fn new(paintable: T) -> Self {
        Self { paintable }
    }
}

impl<T: Paintable + Send + Sync + 'static> egui_wgpu::CallbackTrait for PaintableCallback<T> {
    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &CallbackResources,
    ) {
        // This ugly transmute is needed because the CallbackTrait demands 'static
        // for the render_pass, but we need a shorter lifetime to call our paintable
        // Safe because the render_pass is guaranteed to outlive the paint call
        unsafe {
            let render_pass_with_shorter_lifetime = std::mem::transmute::<
                &mut wgpu::RenderPass<'static>,
                &mut wgpu::RenderPass<'_>,
            >(render_pass);
            self.paintable.paint(render_pass_with_shorter_lifetime);
        }
    }
}

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
