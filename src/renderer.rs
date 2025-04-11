use crate::camera::Camera;
use crate::particle_system::{Particle, ParticleSystem};
use egui_wgpu::wgpu::{Device, RenderPipeline};

pub struct Renderer {
    render_pipeline: RenderPipeline,
    vertex_buffer: egui_wgpu::wgpu::Buffer,
    index_buffer: egui_wgpu::wgpu::Buffer,
    instance_buffer: egui_wgpu::wgpu::Buffer,
    num_indices: u32,
}

impl Renderer {
    pub fn new(
        device: &Device,
        camera: &Camera,
        particle_system: &ParticleSystem,
        sc_desc: &egui_wgpu::wgpu::SurfaceConfiguration,
        shader: &egui_wgpu::wgpu::ShaderModule,
    ) -> Self {
        // Create render pipeline layout
        let render_pipeline_layout =
            device.create_pipeline_layout(&egui_wgpu::wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera.bind_group_layout], // Fixed: Changed bind_group to bind_group_layout
                push_constant_ranges: &[],
            });

        // Create render pipeline
        let render_pipeline =
            device.create_render_pipeline(&egui_wgpu::wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: egui_wgpu::wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"), // Wrapped in Some()
                    buffers: &[
                        // Particle buffer
                        egui_wgpu::wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Particle>()
                                as egui_wgpu::wgpu::BufferAddress,
                            step_mode: egui_wgpu::wgpu::VertexStepMode::Instance,
                            attributes: &[
                                // position
                                egui_wgpu::wgpu::VertexAttribute {
                                    offset: 0,
                                    shader_location: 0,
                                    format: egui_wgpu::wgpu::VertexFormat::Float32x3,
                                },
                                // padding1
                                egui_wgpu::wgpu::VertexAttribute {
                                    offset: std::mem::size_of::<[f32; 3]>()
                                        as egui_wgpu::wgpu::BufferAddress,
                                    shader_location: 1,
                                    format: egui_wgpu::wgpu::VertexFormat::Float32,
                                },
                                // velocity
                                egui_wgpu::wgpu::VertexAttribute {
                                    offset: std::mem::size_of::<[f32; 4]>()
                                        as egui_wgpu::wgpu::BufferAddress,
                                    shader_location: 2,
                                    format: egui_wgpu::wgpu::VertexFormat::Float32x3,
                                },
                                // padding2
                                egui_wgpu::wgpu::VertexAttribute {
                                    offset: std::mem::size_of::<[f32; 7]>()
                                        as egui_wgpu::wgpu::BufferAddress,
                                    shader_location: 3,
                                    format: egui_wgpu::wgpu::VertexFormat::Float32,
                                },
                                // color
                                egui_wgpu::wgpu::VertexAttribute {
                                    offset: std::mem::size_of::<[f32; 8]>()
                                        as egui_wgpu::wgpu::BufferAddress,
                                    shader_location: 4,
                                    format: egui_wgpu::wgpu::VertexFormat::Float32x4,
                                },
                            ],
                        },
                    ],
                    compilation_options: Default::default(), // Added compilation_options
                },
                fragment: Some(egui_wgpu::wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"), // Wrapped in Some()
                    targets: &[Some(egui_wgpu::wgpu::ColorTargetState {
                        format: sc_desc.format,
                        blend: Some(egui_wgpu::wgpu::BlendState {
                            color: egui_wgpu::wgpu::BlendComponent {
                                src_factor: egui_wgpu::wgpu::BlendFactor::SrcAlpha,
                                dst_factor: egui_wgpu::wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: egui_wgpu::wgpu::BlendOperation::Add,
                            },
                            alpha: egui_wgpu::wgpu::BlendComponent {
                                src_factor: egui_wgpu::wgpu::BlendFactor::One,
                                dst_factor: egui_wgpu::wgpu::BlendFactor::One,
                                operation: egui_wgpu::wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: egui_wgpu::wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(), // Added compilation_options
                }),
                primitive: egui_wgpu::wgpu::PrimitiveState {
                    topology: egui_wgpu::wgpu::PrimitiveTopology::PointList,
                    strip_index_format: None,
                    front_face: egui_wgpu::wgpu::FrontFace::Ccw,
                    cull_mode: Some(egui_wgpu::wgpu::Face::Back),
                    polygon_mode: egui_wgpu::wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: egui_wgpu::wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None, // Added cache field
            });

        // Create placeholder buffers for the struct initialization
        let vertex_buffer = device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: 1, // Minimum size
            usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            size: 1, // Minimum size
            usage: egui_wgpu::wgpu::BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        let instance_buffer = device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: 1, // Minimum size
            usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            // Remove the fields we don't need or initialize them properly:
            vertex_buffer: device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
                label: Some("Dummy Vertex Buffer"),
                size: 4,
                usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            index_buffer: device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
                label: Some("Dummy Index Buffer"),
                size: 4,
                usage: egui_wgpu::wgpu::BufferUsages::INDEX,
                mapped_at_creation: false,
            }),
            instance_buffer: device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
                label: Some("Dummy Instance Buffer"),
                size: 4,
                usage: egui_wgpu::wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            num_indices: 0,
        }
    }

    pub fn render(
        &self,
        encoder: &mut egui_wgpu::wgpu::CommandEncoder,
        view: &egui_wgpu::wgpu::TextureView,
        camera: &Camera,
        particle_system: &ParticleSystem,
    ) {
        let mut render_pass = encoder.begin_render_pass(&egui_wgpu::wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(egui_wgpu::wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load: egui_wgpu::wgpu::LoadOp::Clear(egui_wgpu::wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: egui_wgpu::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &camera.bind_group, &[]);
        render_pass.set_vertex_buffer(0, particle_system.particle_buffer.slice(..));
        render_pass.draw(0..1, 0..particle_system.num_particles);
    }
}
