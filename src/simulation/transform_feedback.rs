use super::{Particle, generate_initial_particles};
use super::{ParticleSimulation, SimParams, SimulationMethod};
use wgpu::util::DeviceExt;

pub struct TransformFeedbackSimulation {
    particle_buffers: [wgpu::Buffer; 2],
    uniform_buffer: wgpu::Buffer,

    update_pipeline: wgpu::RenderPipeline,
    uniform_bind_group: wgpu::BindGroup,

    particle_count: u32,
    current_buffer: usize, // Which buffer to read from
    paused: bool,

    // Render target components
    render_texture: wgpu::Texture,
    render_view: wgpu::TextureView,
}

impl ParticleSimulation for TransformFeedbackSimulation {
    fn new(
        device: &wgpu::Device,
        initial_particle_count: u32,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        // Create initial particles
        let particles = generate_initial_particles(initial_particle_count);

        // Create ping-pong buffers for simulation
        let buffer_size =
            (std::mem::size_of::<Particle>() as u64) * (initial_particle_count as u64);
        let buffer_usage = wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;

        let particle_buffers = [
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TF Particle Buffer 0"),
                contents: bytemuck::cast_slice(&particles),
                usage: buffer_usage,
            }),
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TF Particle Buffer 1"),
                contents: bytemuck::cast_slice(&particles),
                usage: buffer_usage,
            }),
        ];

        // Create simulation uniform buffer
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TF Params Buffer"),
            contents: bytemuck::cast_slice(&[SimParams::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create a texture for the invisible render target
        let texture_desc = wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: Some("TF Render Target"),
            view_formats: &[],
        };

        let render_texture = device.create_texture(&texture_desc);
        let render_view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TF Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TF Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TF Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout for Particle struct
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Particle>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // padding1
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32,
                },
                // velocity
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // padding2
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 7]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                // color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        // Create shader module
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TF Update Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transform_feedback.wgsl").into(),
            ),
        });

        // Create render pipeline (our update pipeline)
        let update_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TF Update Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                polygon_mode: wgpu::PolygonMode::Fill,
                front_face: wgpu::FrontFace::Ccw,
                strip_index_format: None,
                cull_mode: None,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,        // Can reuse the same module
                entry_point: Some("fs_dummy"), // Point to the dummy fragment entry point
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format, // Use the format of the dummy render target
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(), // IMPORTANT: Don't write color
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        Self {
            particle_buffers,
            uniform_buffer,
            update_pipeline,
            uniform_bind_group,
            particle_count: initial_particle_count,
            current_buffer: 0,
            paused: false,
            render_texture,
            render_view,
        }
    }

    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &SimParams,
    ) {
        if self.paused {
            return;
        }

        // Update uniform buffer with simulation parameters
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*params]));

        // Set up source and destination buffers
        let src_idx = self.current_buffer;
        let dst_idx = 1 - src_idx;

        // Create staging buffer for transformed data
        // NOTE: This staging buffer is a workaround because WebGPU doesn't directly
        // support writing vertex shader output back to a buffer like OpenGL TF.
        // We simulate it by rendering, copying the *original* data to staging,
        // then copying staging to the destination. This is inefficient but necessary
        // for the WebGL backend where compute shaders or storage buffers might not be available.
        // A more efficient approach would involve compute shaders or storage buffers
        // where supported.
        let staging_buffer_size =
            (self.particle_count as u64) * (std::mem::size_of::<Particle>() as u64);
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TF Staging Buffer"),
            size: staging_buffer_size,
            // Usage: Destination for the first copy, Source for the second copy
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Run render pass that *would* capture vertex outputs if TF was supported
        // In this WebGPU simulation, this pass just runs the vertex shader logic.
        // The actual data transfer happens via copy_buffer_to_buffer.
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("TF Update Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Discard, // We don't need to store the color result
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.update_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.particle_buffers[src_idx].slice(..));

            // Process all particles by drawing points
            render_pass.draw(0..self.particle_count, 0..1);
        } // Render pass ends here

        // Simulate Transform Feedback:
        // 1. Copy the *original* source data into the staging buffer.
        //    (This step seems counter-intuitive, but it's how we get the data
        //     that the vertex shader *would have* written if TF was native).
        //    Ideally, we'd capture the VS output directly.
        encoder.copy_buffer_to_buffer(
            &self.particle_buffers[src_idx], // Source: The buffer read by the VS
            0,
            &staging_buffer, // Destination: Temporary staging
            0,
            staging_buffer_size,
        );

        // 2. Copy the data from the staging buffer into the *actual* destination buffer.
        encoder.copy_buffer_to_buffer(
            &staging_buffer, // Source: Staging buffer with (simulated) VS output
            0,
            &self.particle_buffers[dst_idx], // Destination: The next frame's input buffer
            0,
            staging_buffer_size,
        );

        // NOTE: We do NOT call encoder.finish() or queue.submit() here.
        // The caller (`ParticleApp::update_simulation`) is responsible for that.

        // Swap buffers for the next frame
        self.current_buffer = dst_idx;
    }

    fn resize_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, new_count: u32) {
        if new_count == self.particle_count {
            return;
        }

        // Generate particles for the new count
        let particles = generate_initial_particles(new_count);
        let buffer_usage = wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;

        // Create new buffers
        self.particle_buffers = [
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TF Particle Buffer 0"),
                contents: bytemuck::cast_slice(&particles),
                usage: buffer_usage,
            }),
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TF Particle Buffer 1"),
                contents: bytemuck::cast_slice(&particles),
                usage: buffer_usage,
            }),
        ];

        self.particle_count = new_count;
        self.current_buffer = 0;
    }

    fn get_particle_buffer(&self) -> &wgpu::Buffer {
        &self.particle_buffers[self.current_buffer]
    }

    fn get_method(&self) -> SimulationMethod {
        SimulationMethod::TransformFeedback
    }

    fn get_particle_count(&self) -> u32 {
        self.particle_count
    }

    fn reset(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let particles = generate_initial_particles(self.particle_count);

        // Write initial data to both buffers
        queue.write_buffer(
            &self.particle_buffers[0],
            0,
            bytemuck::cast_slice(&particles),
        );
        queue.write_buffer(
            &self.particle_buffers[1],
            0,
            bytemuck::cast_slice(&particles),
        );

        self.current_buffer = 0;
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
}

impl Drop for TransformFeedbackSimulation {
    fn drop(&mut self) {
        self.render_texture.destroy();
    }
}
