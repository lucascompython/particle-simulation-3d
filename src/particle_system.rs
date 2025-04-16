use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Particle {
    pub position: [f32; 3],
    pub padding1: f32,
    pub velocity: [f32; 3],
    pub padding2: f32,
    pub color: [f32; 4],
}

impl Particle {
    fn new(position: Vec3, velocity: Vec3, color: Vec4) -> Self {
        Self {
            position: position.into(),
            padding1: 0.0,
            velocity: velocity.into(),
            padding2: 0.0,
            color: color.into(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
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
}

pub struct ParticleSystem {
    pub particle_buffer: wgpu::Buffer,
    pub particle_buffer_staging: wgpu::Buffer,
    pub num_particles: u32,
    pub max_particles: u32,
    pub compute_pipeline: wgpu::ComputePipeline,
    pub compute_bind_group: wgpu::BindGroup,
    pub paused: bool,
    pub gravity: f32,
    pub particle_size: f32,
    pub color_mode: u32,
    pub mouse_force: f32,
    pub mouse_radius: f32,
    pub mouse_position: [f32; 3],
    pub is_mouse_dragging: bool,
    pub sim_param_buffer: wgpu::Buffer,
}

impl ParticleSystem {
    pub fn new(
        device: &wgpu::Device,
        max_particles: u32,
        compute_shader: &wgpu::ShaderModule,
    ) -> Self {
        let particle_size = std::mem::size_of::<Particle>() as wgpu::BufferAddress;
        let total_size = particle_size * max_particles as wgpu::BufferAddress;

        // Create initial particles
        let mut particles = Vec::with_capacity(max_particles as usize);
        for _ in 0..max_particles {
            let r = rand::random::<f32>();
            let g = rand::random::<f32>();
            let b = rand::random::<f32>();

            let phi = rand::random::<f32>() * std::f32::consts::PI * 2.0;
            let theta = (rand::random::<f32>() - 0.5) * std::f32::consts::PI;
            let radius = rand::random::<f32>() * 50.0;

            let pos = Vec3::new(
                radius * phi.cos() * theta.cos(),
                radius * theta.sin(),
                radius * phi.sin() * theta.cos(),
            );

            let vel = Vec3::new(
                (rand::random::<f32>() - 0.5) * 0.1,
                (rand::random::<f32>() - 0.5) * 0.1,
                (rand::random::<f32>() - 0.5) * 0.1,
            );

            particles.push(Particle::new(pos, vel, Vec4::new(r, g, b, 1.0)));
        }

        // Create particle buffer
        let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Buffer"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
        });

        // Create staging buffer for readback
        let particle_buffer_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Staging Buffer"),
            size: total_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create compute pipeline for particle updates
        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    // Particle buffer
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
                    // Simulation parameters
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

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: compute_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(), // Add this field
            cache: None,                             // Add this field
        });

        // Create simulation parameters buffer
        let sim_params = SimParams {
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
        };

        let sim_param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Simulation Params Buffer"),
            contents: bytemuck::cast_slice(&[sim_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &compute_bind_group_layout,
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

        Self {
            particle_buffer,
            particle_buffer_staging,
            num_particles: max_particles,
            max_particles,
            compute_pipeline,
            compute_bind_group,
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

    pub fn reset(&mut self, queue: &wgpu::Queue) {
        let mut particles = Vec::with_capacity(self.max_particles as usize);
        for _ in 0..self.max_particles {
            let r = rand::random::<f32>();
            let g = rand::random::<f32>();
            let b = rand::random::<f32>();

            let phi = rand::random::<f32>() * std::f32::consts::PI * 2.0;
            let theta = (rand::random::<f32>() - 0.5) * std::f32::consts::PI;
            let radius = rand::random::<f32>() * 50.0;

            let pos = Vec3::new(
                radius * phi.cos() * theta.cos(),
                radius * theta.sin(),
                radius * phi.sin() * theta.cos(),
            );

            let vel = Vec3::new(
                (rand::random::<f32>() - 0.5) * 0.1,
                (rand::random::<f32>() - 0.5) * 0.1,
                (rand::random::<f32>() - 0.5) * 0.1,
            );

            particles.push(Particle::new(pos, vel, Vec4::new(r, g, b, 1.0)));
        }

        queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&particles));
        self.gravity = 0.0;
    }

    pub fn update(&self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder) {
        if self.paused {
            return;
        }

        // Update simulation parameters
        let sim_params = SimParams {
            delta_time: 0.016, // Fixed timestep for predictable physics
            gravity: self.gravity,
            num_particles: self.num_particles,
            color_mode: self.color_mode,
            mouse_force: self.mouse_force,
            mouse_radius: self.mouse_radius,
            mouse_position_x: self.mouse_position[0],
            mouse_position_y: self.mouse_position[1],
            mouse_position_z: self.mouse_position[2],
            is_mouse_dragging: if self.is_mouse_dragging { 1 } else { 0 },
        };

        queue.write_buffer(
            &self.sim_param_buffer,
            0,
            bytemuck::cast_slice(&[sim_params]),
        );

        // Create compute pass
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Particle Compute Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);

        // Dispatch one workgroup per 128 particles
        let workgroup_count = ((self.num_particles as f32) / 128.0).ceil() as u32;
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }

    // pub fn update(&self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder) {
    //     if self.paused {
    //         return;
    //     }

    //     // Update simulation parameters
    //     let sim_params = SimParams {
    //         delta_time: 0.016, // Fixed timestep for predictable physics
    //         gravity: self.gravity,
    //         num_particles: self.num_particles,
    //         color_mode: self.color_mode,
    //         mouse_force: self.mouse_force,
    //         mouse_radius: self.mouse_radius,
    //         mouse_position_x: self.mouse_position[0],
    //         mouse_position_y: self.mouse_position[1],
    //         mouse_position_z: self.mouse_position[2],
    //         is_mouse_dragging: if self.is_mouse_dragging { 1 } else { 0 },
    //     };

    //     queue.write_buffer(
    //         &self.sim_param_buffer,
    //         0,
    //         bytemuck::cast_slice(&[sim_params]),
    //     );

    //     // Create compute pass
    //     let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
    //         label: Some("Particle Compute Pass"),
    //         timestamp_writes: None,
    //     });

    //     compute_pass.set_pipeline(&self.compute_pipeline);
    //     compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);

    //     // Dispatch one workgroup per 128 particles
    //     let workgroup_count = ((self.num_particles as f32) / 128.0).ceil() as u32;
    //     compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    // }
}
