use super::{SphereGeneration, generate_initial_particles};

use super::{ParticleSimulation, SimParams, SimulationMethod};
use wgpu::util::DeviceExt;

pub struct ComputeParticleSimulation {
    particle_buffer: wgpu::Buffer,
    sim_param_buffer: wgpu::Buffer,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    particle_count: u32,
    paused: bool,
    generation_mode: SphereGeneration,
}

impl ParticleSimulation for ComputeParticleSimulation {
    fn new(
        device: &wgpu::Device,
        initial_particle_count: u32,
        _surface_format: wgpu::TextureFormat,
        generation_mode: SphereGeneration,
    ) -> Self {
        // Create initial particles
        let particles = generate_initial_particles(initial_particle_count, generation_mode);

        // Create particle buffer
        let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Compute Particle Buffer"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
        });

        // Create simulation parameters buffer
        let sim_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Compute Sim Params Buffer"),
            size: std::mem::size_of::<SimParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create compute shader
        let compute_shader = unsafe {
            device.create_shader_module_trusted(
                wgpu::include_wgsl!("../shaders/compute.wgsl"),
                wgpu::ShaderRuntimeChecks::unchecked(),
            )
        };

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create bind group
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sim_param_buffer.as_entire_binding(),
                },
            ],
        });

        // Create compute pipeline
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            particle_buffer,
            sim_param_buffer,
            compute_pipeline,
            compute_bind_group,
            bind_group_layout,
            particle_count: initial_particle_count,
            paused: false,
            generation_mode,
        }
    }

    fn update(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &SimParams,
    ) {
        queue.write_buffer(&self.sim_param_buffer, 0, bytemuck::cast_slice(&[*params]));

        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Particle Compute Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);

        // dispatch one workgroup per 128 particles
        let workgroup_count = self.particle_count.div_ceil(256);
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }

    fn resize_buffer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        new_count: u32,
        generation_mode: SphereGeneration,
    ) {
        self.generation_mode = generation_mode;

        if new_count == self.particle_count {
            return;
        }

        // Generate particles for the new count
        let particles = generate_initial_particles(new_count, generation_mode);

        if new_count > self.particle_count {
            // Create new buffer
            self.particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Compute Particle Buffer"),
                contents: bytemuck::cast_slice(&particles),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::VERTEX,
            });

            // Create new bind group with the new buffer
            self.compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.particle_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.sim_param_buffer.as_entire_binding(),
                    },
                ],
            });
        } else {
            queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&particles));
        }

        // Update instance fields
        self.particle_count = new_count;
    }

    fn get_particle_buffer(&self) -> &wgpu::Buffer {
        &self.particle_buffer
    }

    fn get_method(&self) -> SimulationMethod {
        SimulationMethod::ComputeShader
    }

    fn get_particle_count(&self) -> u32 {
        self.particle_count
    }
    fn reset(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        generation_mode: SphereGeneration,
    ) {
        self.generation_mode = generation_mode;
        let particles = generate_initial_particles(self.particle_count, generation_mode);

        queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&particles));
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
}
