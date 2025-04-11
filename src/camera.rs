use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use std::f32::consts::PI;
use winit::keyboard::KeyCode;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [f32; 16],
    pub position: [f32; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array(),
            position: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub up: Vec3,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    pub movement_speed: f32,
    pub rotation_speed: f32,
    pub uniform: CameraUniform,
    pub buffer: egui_wgpu::wgpu::Buffer,
    pub bind_group_layout: egui_wgpu::wgpu::BindGroupLayout,
    pub bind_group: egui_wgpu::wgpu::BindGroup,
}

impl Camera {
    pub fn new(device: &egui_wgpu::wgpu::Device, width: u32, height: u32) -> Self {
        let aspect = width as f32 / height as f32;
        let uniform = CameraUniform::default();

        let buffer = device.create_buffer_init(&egui_wgpu::wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: egui_wgpu::wgpu::BufferUsages::UNIFORM | egui_wgpu::wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&egui_wgpu::wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[egui_wgpu::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: egui_wgpu::wgpu::ShaderStages::VERTEX
                        | egui_wgpu::wgpu::ShaderStages::FRAGMENT,
                    ty: egui_wgpu::wgpu::BindingType::Buffer {
                        ty: egui_wgpu::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let bind_group = device.create_bind_group(&egui_wgpu::wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &bind_group_layout,
            entries: &[egui_wgpu::wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        let mut camera = Self {
            position: Vec3::new(0.0, 0.0, 100.0),
            yaw: -PI / 2.0,
            pitch: 0.0,
            up: Vec3::Y,
            fov: PI / 3.0,
            aspect,
            near: 0.1,
            far: 1000.0,
            movement_speed: 50.0,
            rotation_speed: 0.003,
            uniform,
            buffer,
            bind_group_layout,
            bind_group,
        };

        camera.update_view_proj();
        camera
    }

    pub fn update_view_proj(&mut self) {
        // Create view matrix
        let forward = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize();

        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);

        let view = Mat4::look_at_rh(self.position, self.position + forward, up);
        let proj = Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far);

        self.uniform.view_proj = (proj * view).to_cols_array();
        self.uniform.position = [self.position.x, self.position.y, self.position.z, 1.0];
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
        self.update_view_proj();
    }

    pub fn process_keyboard(&mut self, key: KeyCode, dt: f32) -> bool {
        let mut moved = false;

        let forward = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize();

        let right = forward.cross(Vec3::Y).normalize();
        let up = Vec3::Y;

        let speed = self.movement_speed * dt;

        match key {
            KeyCode::KeyW => {
                self.position += forward * speed;
                moved = true;
            }
            KeyCode::KeyS => {
                self.position -= forward * speed;
                moved = true;
            }
            KeyCode::KeyA => {
                self.position -= right * speed;
                moved = true;
            }
            KeyCode::KeyD => {
                self.position += right * speed;
                moved = true;
            }
            KeyCode::Space => {
                self.position += up * speed;
                moved = true;
            }
            KeyCode::ShiftLeft => {
                self.position -= up * speed;
                moved = true;
            }
            _ => {}
        }

        if moved {
            self.update_view_proj();
        }

        moved
    }

    pub fn process_mouse_movement(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * self.rotation_speed;
        self.pitch =
            (self.pitch - dy * self.rotation_speed).clamp(-PI / 2.0 + 0.01, PI / 2.0 - 0.01);

        self.update_view_proj();
    }

    pub fn update_buffer(&self, queue: &egui_wgpu::wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}
