use super::ParticleSimulator;
use glam::{Vec3, Vec4};
use rand::{Rng, SeedableRng, rngs::SmallRng};
use wgpu::util::DeviceExt;

// Use the same Particle structure as compute version
use super::Particle;

pub struct FragmentParticleSystem {
    // Particle data
    particle_buffer: wgpu::Buffer,
    texture_size: wgpu::Extent3d,
    particle_textures: [wgpu::Texture; 2],
    particle_texture_views: [wgpu::TextureView; 2],
    particle_samplers: [wgpu::Sampler; 2],

    // Ping-pong rendering resources
    render_bind_groups: [wgpu::BindGroup; 2],
    render_pipeline: wgpu::RenderPipeline,
    current_texture: usize, // 0 or 1, which texture is "current"

    // Output vertex buffer (for rendering particles)
    output_buffer: wgpu::Buffer,
    output_bind_group: wgpu::BindGroup,

    // Simulation parameters (same as compute version)
    max_particles: u32,
    num_particles: u32,
    paused: bool,
    gravity: f32,
    particle_size: f32,
    color_mode: u32,
    mouse_force: f32,
    mouse_radius: f32,
    mouse_position: [f32; 3],
    is_mouse_dragging: bool,
    sim_param_buffer: wgpu::Buffer,
}

impl FragmentParticleSystem {
    pub fn new(
        device: &wgpu::Device,
        max_particles: u32,
        fragment_shader: &wgpu::ShaderModule,
    ) -> Self {
        // Calculate texture dimensions needed to store particles
        // Each texel will store one particle (RGBA32F x 3 textures)
        let texture_width = (max_particles as f32).sqrt().ceil() as u32;
        let texture_height = ((max_particles as f64) / (texture_width as f64)).ceil() as u32;

        let texture_size = wgpu::Extent3d {
            width: texture_width,
            height: texture_height,
            depth_or_array_layers: 1,
        };

        // Create particle textures (ping-pong pair)
        let particle_textures = [
            create_particle_texture(device, texture_size, "Particle Texture A"),
            create_particle_texture(device, texture_size, "Particle Texture B"),
        ];

        let particle_texture_views = [
            particle_textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            particle_textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let particle_samplers = [
            create_particle_sampler(device, "Particle Sampler A"),
            create_particle_sampler(device, "Particle Sampler B"),
        ];

        // Create initial particle data using SmallRng
        let mut rng = SmallRng::seed_from_u64(42);
        let mut particles = Vec::with_capacity(max_particles as usize);

        for _ in 0..max_particles {
            let r = rng.random::<f32>();
            let g = rng.random::<f32>();
            let b = rng.random::<f32>();

            let phi = rng.random::<f32>() * std::f32::consts::PI * 2.0;
            let theta = (rng.random::<f32>() - 0.5) * std::f32::consts::PI;
            let radius = rng.random::<f32>() * 50.0;

            let pos = Vec3::new(
                radius * phi.cos() * theta.cos(),
                radius * theta.sin(),
                radius * phi.sin() * theta.cos(),
            );

            let vel = Vec3::new(
                (rng.random::<f32>() - 0.5) * 0.1,
                (rng.random::<f32>() - 0.5) * 0.1,
                (rng.random::<f32>() - 0.5) * 0.1,
            );

            particles.push(Particle::new(pos, vel, Vec4::new(r, g, b, 1.0)));
        }

        // Create vertex buffer for particle rendering
        let output_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Output Buffer"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
        });

        // Create simulation param buffer
        let sim_param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Simulation Params Buffer"),
            contents: bytemuck::cast_slice(&[SimParams {
                delta_time: 0.016,
                gravity: 0.0,
                num_particles: max_particles,
                color_mode: 0,
                mouse_force: 5.0,
                mouse_radius: 10.0,
                mouse_position_x: 0.0,
                mouse_position_y: 0.0,
                mouse_position_z: 0.0,
                is_mouse_dragging: 0,
                texture_width: texture_width as f32,
                texture_height: texture_height as f32,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layout for the render pipeline
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Render Bind Group Layout"),
                entries: &[
                    // Source texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    // Simulation parameters
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Output buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create bind groups for ping-pong rendering
        let render_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render Bind Group A"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&particle_texture_views[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&particle_samplers[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_param_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render Bind Group B"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&particle_texture_views[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&particle_samplers[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_param_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        // Create output bind group for copying to vertex buffer
        let output_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Output Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Output Bind Group"),
            layout: &output_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: output_buffer.as_entire_binding(),
            }],
        });

        // Create render pipeline for ping-pong
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Update Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: fragment_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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

        // Initialize textures with initial particle data
        // This would need code to upload the initial particle data to textures
        // For simplicity, we'll skip that here and assume the first frame handles it

        Self {
            particle_buffer: output_buffer.clone(),
            texture_size,
            particle_textures,
            particle_texture_views,
            particle_samplers,
            render_bind_groups,
            render_pipeline,
            current_texture: 0,
            output_buffer,
            output_bind_group,
            max_particles,
            num_particles: max_particles,
            paused: false,
            gravity: 0.0,
            particle_size: 1.0,
            color_mode: 0,
            mouse_force: 5.0,
            mouse_radius: 10.0,
            mouse_position: [0.0, 0.0, 0.0],
            is_mouse_dragging: false,
            sim_param_buffer,
        }
    }
}

// Helper functions
fn create_particle_texture(
    device: &wgpu::Device,
    size: wgpu::Extent3d,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_particle_sampler(device: &wgpu::Device, label: &str) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    })
}

// Extended SimParams with texture dimensions
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SimParams {
    delta_time: f32,
    gravity: f32,
    num_particles: u32,
    color_mode: u32,
    mouse_force: f32,
    mouse_radius: f32,
    mouse_position_x: f32,
    mouse_position_y: f32,
    mouse_position_z: f32,
    is_mouse_dragging: u32,
    texture_width: f32,
    texture_height: f32,
}

impl ParticleSimulator for FragmentParticleSystem {
    fn update(&mut self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder) {
        if self.paused {
            return;
        }

        // Update simulation parameters
        let sim_params = SimParams {
            delta_time: 0.016,
            gravity: self.gravity,
            num_particles: self.num_particles,
            color_mode: self.color_mode,
            mouse_force: self.mouse_force,
            mouse_radius: self.mouse_radius,
            mouse_position_x: self.mouse_position[0],
            mouse_position_y: self.mouse_position[1],
            mouse_position_z: self.mouse_position[2],
            is_mouse_dragging: if self.is_mouse_dragging { 1 } else { 0 },
            texture_width: self.texture_size.width as f32,
            texture_height: self.texture_size.height as f32,
        };

        queue.write_buffer(
            &self.sim_param_buffer,
            0,
            bytemuck::cast_slice(&[sim_params]),
        );

        // Source is current texture, target is the other one
        let source = self.current_texture;
        let target = 1 - source;

        // Render to the target texture
        let target_view = &self.particle_texture_views[target];
        let texture_view = target_view;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Particle Update Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Use the source texture's bind group to read from it
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.render_bind_groups[source], &[]);

        // Draw a full-screen quad to process all particles
        render_pass.draw(0..4, 0..1);

        // Flip which texture is current for next frame
        self.current_texture = target;
    }

    fn reset(&mut self, queue: &wgpu::Queue) {
        // For the fragment shader implementation, we would need to reset
        // the textures with initial particle data. This is more complex than
        // the compute shader version and would require encoding more commands.

        // For simplicity in this example, we'll just update parameters
        self.gravity = 0.0;

        // In a real implementation, we would upload new texture data
        // or run a special reset shader
    }

    fn get_vertex_buffer(&self) -> &wgpu::Buffer {
        &self.particle_buffer
    }

    fn get_max_particles(&self) -> u32 {
        self.max_particles
    }

    fn get_num_particles(&self) -> u32 {
        self.num_particles
    }

    fn set_num_particles(&mut self, num: u32) {
        self.num_particles = num.min(self.max_particles);
    }

    fn get_gravity(&self) -> f32 {
        self.gravity
    }

    fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    fn get_color_mode(&self) -> u32 {
        self.color_mode
    }

    fn set_color_mode(&mut self, mode: u32) {
        self.color_mode = mode;
    }

    fn get_mouse_position(&self) -> [f32; 3] {
        self.mouse_position
    }

    fn set_mouse_position(&mut self, pos: [f32; 3]) {
        self.mouse_position = pos;
    }

    fn get_mouse_force(&self) -> f32 {
        self.mouse_force
    }

    fn set_mouse_force(&mut self, force: f32) {
        self.mouse_force = force;
    }

    fn get_mouse_radius(&self) -> f32 {
        self.mouse_radius
    }

    fn set_mouse_radius(&mut self, radius: f32) {
        self.mouse_radius = radius;
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    fn set_mouse_dragging(&mut self, dragging: bool) {
        self.is_mouse_dragging = dragging;
    }

    fn is_mouse_dragging(&self) -> bool {
        self.is_mouse_dragging
    }

    fn get_particle_size(&self) -> f32 {
        self.particle_size
    }

    fn set_particle_size(&mut self, size: f32) {
        self.particle_size = size;
    }

    fn get_particle_buffer(&self) -> &wgpu::Buffer {
        &self.particle_buffer
    }
}
