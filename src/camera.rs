use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::f32::consts::PI;
use wgpu::util::DeviceExt;

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
    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl Camera {
    pub fn new(device: &wgpu::Device, aspect: f32) -> Self {
        let uniform = CameraUniform::default();

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
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
        let forward = self.get_forward();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);

        let view = Mat4::look_at_rh(self.position, self.position + forward, up);
        let proj = Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far);

        self.uniform.view_proj = (proj * view).to_cols_array();
        self.uniform.position = [self.position.x, self.position.y, self.position.z, 1.0];
    }

    pub fn get_forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn get_right(&self) -> Vec3 {
        self.get_forward().cross(Vec3::Y).normalize()
    }

    pub fn get_up(&self) -> Vec3 {
        self.get_right().cross(self.get_forward())
    }

    pub fn process_keyboard(&mut self, key: egui::Key, shift_down: bool, dt: f32) -> bool {
        let mut moved = false;

        let forward = self.get_forward();
        let right = self.get_right();
        let up = Vec3::Y;

        let speed = self.movement_speed * dt;

        match key {
            egui::Key::W => {
                self.position += forward * speed;
                moved = true;
            }
            egui::Key::S => {
                self.position -= forward * speed;
                moved = true;
            }
            egui::Key::A => {
                self.position -= right * speed;
                moved = true;
            }
            egui::Key::D => {
                self.position += right * speed;
                moved = true;
            }
            egui::Key::Space => {
                if shift_down {
                    self.position -= up * speed;
                } else {
                    self.position += up * speed;
                }
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

    pub fn update_buffer(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}
