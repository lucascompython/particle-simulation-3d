use super::{Particle, generate_initial_particles};

use super::{ParticleSimulation, SimParams, SimulationMethod};
use wgpu::util::DeviceExt;

pub struct TransformFeedbackSimulation {
    particle_buffers: [wgpu::Buffer; 2],
    current_buffer: usize,
    uniform_buffer: wgpu::Buffer,
    bind_groups: [wgpu::BindGroup; 2],
    bind_group_layout: wgpu::BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
    particle_count: u32,
    paused: bool,
}

impl ParticleSimulation for TransformFeedbackSimulation {
    fn new(device: &wgpu::Device, initial_particle_count: u32) -> Self {
        let particles = generate_initial_particles(initial_particle_count);

        // Create ping-pong buffers
        let buffer_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TF Particle Buffer A"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
        });

        let buffer_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TF Particle Buffer B"),
            size: (std::mem::size_of::<Particle>() as u64) * (initial_particle_count as u64),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        // Create uniform buffer for simulation parameters
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TF Simulation Params"),
            size: std::mem::size_of::<SimParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create shader for transform feedback (vertex shader based simulation)
        let tf_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Transform Feedback Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/transform_feedback.wgsl").into(),
            ),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TF Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create bind groups for ping-ponging
        let bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TF Bind Group A"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_b.as_entire_binding(),
                },
            ],
        });

        let bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TF Bind Group B"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_a.as_entire_binding(),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TF Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline (this is not for display but for simulation)
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TF Simulation Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &tf_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: None, // We don't need fragment shader for simulation
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            particle_buffers: [buffer_a, buffer_b],
            current_buffer: 0,
            uniform_buffer,
            bind_groups: [bind_group_a, bind_group_b],
            bind_group_layout,
            render_pipeline,
            particle_count: initial_particle_count,
            paused: false,
        }
    }

    fn update(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &SimParams,
    ) {
        // Update simulation parameters
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*params]));

        // Create render pass for simulation (not actually rendering to screen)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("TF Simulation Pass"),
                color_attachments: &[],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_groups[self.current_buffer], &[]);
            render_pass.draw(0..self.particle_count, 0..1);
        }

        // Swap buffers for next frame
        self.current_buffer = 1 - self.current_buffer;
    }

    fn resize_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, new_count: u32) {
        if new_count == self.particle_count {
            return;
        }

        // Generate initial particles for the new count
        let particles = generate_initial_particles(new_count);

        // Create new buffers with the appropriate size
        let buffer_size = (std::mem::size_of::<Particle>() as u64) * (new_count as u64);

        let buffer_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TF Particle Buffer A"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
        });

        let buffer_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TF Particle Buffer B"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        // Create bind groups for ping-ponging with new buffers
        let bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TF Bind Group A"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_b.as_entire_binding(),
                },
            ],
        });

        let bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TF Bind Group B"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer_b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_a.as_entire_binding(),
                },
            ],
        });

        // Update struct fields
        self.particle_buffers = [buffer_a, buffer_b];
        self.bind_groups = [bind_group_a, bind_group_b];
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
        // Generate initial particles
        let particles = generate_initial_particles(self.particle_count);

        // Update buffer A with new particles
        queue.write_buffer(
            &self.particle_buffers[0],
            0,
            bytemuck::cast_slice(&particles),
        );

        // Reset to buffer 0 as current
        self.current_buffer = 0;
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
}
