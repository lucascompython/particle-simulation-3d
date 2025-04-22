use super::{Particle, generate_initial_particles};

use super::{ParticleSimulation, SimParams, SimulationMethod};
use wgpu::util::DeviceExt;

pub struct TransformFeedbackSimulation {
    particle_buffers: [wgpu::Buffer; 2],
    uniform_buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    uniform_bind_group: wgpu::BindGroup,
    particle_count: u32,
    current_buffer_idx: usize,
    paused: bool,

    dummy_texture: wgpu::Texture,
    dummy_texture_view: wgpu::TextureView,
}

impl ParticleSimulation for TransformFeedbackSimulation {
    fn new(
        device: &wgpu::Device,
        initial_particle_count: u32,
        // TODO: See if its possible to make surface_format specific to Tranform Feedback
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let particles = generate_initial_particles(initial_particle_count);
        let buffer_size =
            (std::mem::size_of::<Particle>() as u64) * (initial_particle_count as u64);

        // Create ping-pong buffers
        // Usage:
        // - VERTEX: Input to the simulation vertex shader, and input to the final render pass.
        // - STORAGE: wgpu might require this for TF emulation or internal mechanisms.
        // - COPY_DST: To allow resetting/resizing by writing new data.
        // - COPY_SRC: Potentially useful for debugging, but not strictly required now.
        let buffer_usage =
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

        let particle_buffers = [
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TF Particle Buffer 0"),
                contents: bytemuck::cast_slice(&particles),
                usage: buffer_usage,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("TF Particle Buffer 1"),
                size: buffer_size,
                usage: buffer_usage,
                mapped_at_creation: false,
            }),
        ];

        // Uniform buffer for simulation parameters
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TF SimParams Buffer"),
            size: std::mem::size_of::<SimParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dummy_texture_desc = wgpu::TextureDescriptor {
            label: Some("TF Dummy Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format, // Use a known format
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, // Essential usage
            view_formats: &[],
        };
        let dummy_texture = device.create_texture(&dummy_texture_desc);
        let dummy_texture_view = dummy_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // --- Shader and Pipeline Setup ---
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TF Simulation Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transform_feedback.wgsl").into(),
            ),
        });

        // Bind group layout for uniforms
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("TF Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX, // Only used in vertex shader
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<SimParams>() as _
                        ),
                    },
                    count: None,
                }],
            });

        // Bind group for uniforms
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TF Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TF Simulation Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout describing the Particle struct
        let particle_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Particle>() as wgpu::BufferAddress,
            // Step mode INSTANCE is wrong here, we want one vertex shader invocation per particle.
            // Use VERTEX step mode, and draw `particle_count` vertices.
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Match Particle struct layout and shader @location inputs
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0, // Corresponds to @location(0) in shader
                    format: wgpu::VertexFormat::Float32x3,
                },
                // padding1
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32, // Assuming padding is f32
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
                    format: wgpu::VertexFormat::Float32, // Assuming padding is f32
                },
                // color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        // Create the simulation pipeline
        // This pipeline reads from a vertex buffer and uses Transform Feedback
        // (implicitly via wgpu based on shader outputs and no fragment stage)
        // to write to another buffer.
        // let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        //     label: Some("TF Simulation Pipeline"),
        //     layout: Some(&pipeline_layout),
        //     vertex: wgpu::VertexState {
        //         module: &shader_module,
        //         entry_point: Some("vs_main"),
        //         buffers: &[particle_buffer_layout], // Describes the input vertex buffer
        //         compilation_options: Default::default(),
        //     },
        //     // fragment: None, // No fragment shader needed for simulation pass
        //     fragment: Some(wgpu::FragmentState {
        //         module: &shader_module,             // Reuse the same shader module
        //         entry_point: Some("fs_dummy_main"), // Reference the dummy entry point we added
        //         targets: &[Some(wgpu::ColorTargetState {
        //             format: surface_format, // Match the dummy texture format used in the pass
        //             blend: None,            // No blending needed
        //             write_mask: wgpu::ColorWrites::empty(), // IMPORTANT: Prevent any actual color writes
        //         })],
        //         compilation_options: Default::default(),
        //     }),
        //     primitive: wgpu::PrimitiveState {
        //         topology: wgpu::PrimitiveTopology::PointList, // Process one vertex per particle
        //         ..Default::default()
        //     },
        //     depth_stencil: Some(wgpu::DepthStencilState {
        //         format: wgpu::TextureFormat::Depth32Float, // Or Depth24Plus, Depth24PlusStencil8
        //         depth_write_enabled: false,                // Don't write depth
        //         depth_compare: wgpu::CompareFunction::Always, // Don't actually test depth
        //         stencil: wgpu::StencilState::default(),    // Default stencil state (disabled)
        //         bias: wgpu::DepthBiasState::default(),
        //     }),

        //     multisample: wgpu::MultisampleState::default(), // No multisampling needed
        //     multiview: None,
        //     cache: None,
        // });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TF Simulation Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[particle_buffer_layout],
                compilation_options: Default::default(),
            },
            // Keep the FragmentState with dummy entry point and color target declaration
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_dummy_main"), // Reference the dummy entry point
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format, // Match the dummy texture format
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(), // Prevent actual writes
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                ..Default::default()
            },
            // *** REMOVE the depth_stencil field ***
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            particle_buffers,
            uniform_buffer,
            pipeline,
            uniform_bind_group,
            particle_count: initial_particle_count,
            current_buffer_idx: 0,
            paused: false,
            dummy_texture,
            dummy_texture_view,
        }
    }

    fn update(
        &mut self,
        device: &wgpu::Device, // Added device argument
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &SimParams,
    ) {
        if self.paused || self.particle_count == 0 {
            return;
        }

        // Update simulation parameters
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*params]));

        let input_buffer_idx = self.current_buffer_idx;
        let output_buffer_idx = 1 - input_buffer_idx;

        // Create render pass for simulation (no color/depth attachments)
        {
            let mut sim_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("TF Simulation Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.dummy_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // LoadOp::Clear or LoadOp::Load are fine. Clear might be marginally better.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        // StoreOp::Discard is important as we don't need the result.
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            sim_pass.set_pipeline(&self.pipeline);
            sim_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Set the INPUT vertex buffer
            sim_pass.set_vertex_buffer(
                0, // Corresponds to buffers[0] in pipeline descriptor
                self.particle_buffers[input_buffer_idx].slice(..),
            );

            // Set the OUTPUT buffer for Transform Feedback
            // Use set_vertex_buffer with a slot index >= number of input buffers
            // Slot 1 here corresponds to vertex_output_buffer_layouts[0]
            sim_pass.set_vertex_buffer(
                1, // Slot for the *output* buffer
                self.particle_buffers[output_buffer_idx].slice(..),
            );

            // Draw particle_count points, invoking the vertex shader for each
            sim_pass.draw(0..self.particle_count, 0..1);
        } // sim_pass is dropped, recording ends

        // Swap buffers for the next frame
        self.current_buffer_idx = output_buffer_idx;
    }

    fn resize_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, new_count: u32) {
        if new_count == self.particle_count {
            return;
        }
        if new_count == 0 {
            self.particle_count = 0;
            // Optionally destroy buffers here if count goes to 0 permanently
            return;
        }

        let particles = generate_initial_particles(new_count);
        let buffer_size = (std::mem::size_of::<Particle>() as u64) * (new_count as u64);
        let buffer_usage =
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

        // Destroy old buffers before creating new ones
        self.particle_buffers[0].destroy();
        self.particle_buffers[1].destroy();

        self.particle_buffers = [
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("TF Particle Buffer 0"),
                contents: bytemuck::cast_slice(&particles),
                usage: buffer_usage,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("TF Particle Buffer 1"),
                size: buffer_size,
                usage: buffer_usage,
                mapped_at_creation: false,
            }),
        ];

        self.particle_count = new_count;
        self.current_buffer_idx = 0; // Start reading from buffer 0 again
    }

    fn get_particle_buffer(&self) -> &wgpu::Buffer {
        // Return the buffer that currently holds the latest simulation results
        &self.particle_buffers[self.current_buffer_idx]
    }

    fn get_method(&self) -> SimulationMethod {
        SimulationMethod::TransformFeedback
    }

    fn get_particle_count(&self) -> u32 {
        self.particle_count
    }

    fn reset(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.particle_count == 0 {
            return;
        }
        let particles = generate_initial_particles(self.particle_count);
        queue.write_buffer(
            &self.particle_buffers[0], // Write to buffer 0
            0,
            bytemuck::cast_slice(&particles),
        );
        // Optionally clear buffer 1 if needed, though it will be overwritten
        // queue.write_buffer(&self.particle_buffers[1], 0, ...);
        self.current_buffer_idx = 0; // Ensure we read from buffer 0 next
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
        self.dummy_texture.destroy();
    }
}
