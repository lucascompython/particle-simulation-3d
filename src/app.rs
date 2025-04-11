use crate::camera::Camera;
use crate::egui_tools::EguiRenderer;
use crate::input_manager::InputManager;
use crate::particle_system::ParticleSystem;
use crate::renderer::Renderer;

use egui_wgpu::wgpu::SurfaceError;
use egui_wgpu::{ScreenDescriptor, wgpu};
use glam::Vec3;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event::{ElementState, MouseButton};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;
use winit::window::{CursorGrabMode, Window, WindowId};

pub struct AppState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub surface: wgpu::Surface<'static>,
    pub scale_factor: f32,
    pub egui_renderer: EguiRenderer,
    pub particle_system: ParticleSystem,
    pub camera: Camera,
    pub renderer: Renderer,
    pub input_manager: InputManager,
    pub last_update: Instant,
    pub delta_time: f32,
    pub fps: f32,
    pub fps_counter: u32,
    pub fps_timer: f32,
    pub show_ui: bool,
    pub fullscreen: bool,
    pub window: Option<Arc<Window>>,
}

impl AppState {
    async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        window: &Arc<Window>,
        width: u32,
        height: u32,
    ) -> Self {
        let power_pref = wgpu::PowerPreference::HighPerformance;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: power_pref,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let features = wgpu::Features::empty();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: features,
                    required_limits: Default::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let selected_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|d| **d == selected_format)
            .expect("failed to select proper surface texture format!");

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *swapchain_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 0,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        let egui_renderer = EguiRenderer::new(&device, surface_config.format, None, 1, &window);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle.wgsl").into()),
        });

        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl").into()),
        });

        let camera = Camera::new(&device, width, height);

        let max_particles = 1_000_000; // Start with 1 million particles
        let particle_system = ParticleSystem::new(&device, max_particles, &compute_shader);

        let renderer = Renderer::new(&device, &camera, &particle_system, &surface_config, &shader);

        let input_manager = InputManager::new();
        let scale_factor = 1.0;

        Self {
            device,
            queue,
            surface,
            surface_config,
            egui_renderer,
            particle_system,
            camera,
            renderer,
            input_manager,
            last_update: Instant::now(),
            delta_time: 0.016,
            fps: 0.0,
            fps_counter: 0,
            fps_timer: 0.0,
            scale_factor,
            show_ui: true,
            fullscreen: false,
            window: Some(Arc::clone(&window)),
        }
    }

    fn resize_surface(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.camera.resize(width, height);
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        self.delta_time = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        self.fps_counter += 1;
        self.fps_timer += self.delta_time;
        if self.fps_timer >= 1.0 {
            self.fps = self.fps_counter as f32 / self.fps_timer;
            self.fps_counter = 0;
            self.fps_timer = 0.0;
        }

        for key in [
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::Space,
            KeyCode::ShiftLeft,
        ]
        .iter()
        {
            if self.input_manager.is_key_pressed(*key) {
                self.camera.process_keyboard(*key, self.delta_time);
            }
        }

        if self.input_manager.is_mouse_captured() {
            let (dx, dy) = self.input_manager.mouse_delta();
            self.camera.process_mouse_movement(dx, dy);
            self.input_manager.reset_mouse_delta();
        }

        self.camera.update_buffer(&self.queue);

        self.particle_system.is_mouse_dragging = self
            .input_manager
            .is_mouse_button_pressed(MouseButton::Left);

        if self.particle_system.is_mouse_dragging {
            let (x, y) = self.input_manager.mouse_position();

            // Convert screen coordinates to normalized device coordinates (-1 to 1)
            let ndc_x = (2.0 * x / self.surface_config.width as f32) - 1.0;
            let ndc_y = 1.0 - (2.0 * y / self.surface_config.height as f32);

            let camera_forward = Vec3::new(
                self.camera.yaw.cos() * self.camera.pitch.cos(),
                self.camera.pitch.sin(),
                self.camera.yaw.sin() * self.camera.pitch.cos(),
            )
            .normalize();

            let camera_right = camera_forward.cross(Vec3::Y).normalize();
            let camera_up = camera_right.cross(camera_forward).normalize();

            // Use mouse_depth to determine distance from camera
            let distance = 20.0 + self.particle_system.mouse_depth;

            let plane_center = self.camera.position + camera_forward * distance;

            // Scale the NDC coordinates based on the field of view and distance
            let height = 2.0 * distance * (self.camera.fov / 2.0).tan();
            let width = height * self.camera.aspect;

            let world_pos = plane_center
                + camera_right * (ndc_x * width / 2.0)
                + camera_up * (ndc_y * height / 2.0);

            self.particle_system.mouse_position = [world_pos.x, world_pos.y, world_pos.z];
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.particle_system
            .update(&self.queue, &mut encoder, self.delta_time);

        self.renderer
            .render(&mut encoder, &view, &self.camera, &self.particle_system);

        if self.show_ui {
            if let Some(window) = self.window.as_ref() {
                self.egui_renderer.begin_frame(window);

                self.render_ui();

                if let Some(window) = self.window.as_ref() {
                    let screen_descriptor = ScreenDescriptor {
                        size_in_pixels: [self.surface_config.width, self.surface_config.height],
                        pixels_per_point: 1.0 * self.scale_factor, // Use 1.0 as default scale factor
                    };

                    self.egui_renderer.end_frame_and_draw(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        window,
                        &view,
                        screen_descriptor,
                    );
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn render_ui(&mut self) {
        let ctx = self.egui_renderer.context();

        egui::Window::new("Particle Simulator")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Statistics");
                ui.label(format!("FPS: {:.1}", self.fps));

                ui.separator();
                ui.heading("Simulation");

                if ui
                    .button(if self.particle_system.paused {
                        "Resume"
                    } else {
                        "Pause"
                    })
                    .clicked()
                {
                    self.particle_system.paused = !self.particle_system.paused;
                }

                ui.separator();
                ui.heading("Mouse Interaction");
                ui.label(format!(
                    "Position: ({:.2}, {:.2}, {:.2})",
                    self.particle_system.mouse_position[0],
                    self.particle_system.mouse_position[1],
                    self.particle_system.mouse_position[2]
                ));

                ui.label(format!(
                    "Dragging: {}",
                    self.particle_system.is_mouse_dragging
                ));
                ui.label(format!("Depth: {:.2}", self.particle_system.mouse_depth));

                ui.add(
                    egui::Slider::new(&mut self.particle_system.mouse_radius, 1.0..=50.0)
                        .text("Radius"),
                );

                ui.add(
                    egui::Slider::new(&mut self.particle_system.mouse_force, 0.0..=100.0)
                        .text("Force"),
                );

                ui.separator();
                ui.heading("Camera");
                ui.label(format!(
                    "Position: ({:.2}, {:.2}, {:.2})",
                    self.camera.position.x, self.camera.position.y, self.camera.position.z
                ));

                let mut fov_degrees = self.camera.fov * 180.0 / std::f32::consts::PI;
                ui.add(
                    egui::Slider::new(&mut fov_degrees, 10.0..=120.0)
                        .text("Field of View (degrees)"),
                );
                // convert to radians and update camera if changed
                if (fov_degrees * std::f32::consts::PI / 180.0 - self.camera.fov).abs() > 0.001 {
                    self.camera.fov = fov_degrees * std::f32::consts::PI / 180.0;
                    self.camera.update_view_proj();
                }

                ui.separator();
                ui.heading("Particle Settings");

                ui.add(
                    egui::Slider::new(&mut self.particle_system.gravity, 0.0..=5.0).text("Gravity"),
                );

                ui.add(
                    egui::Slider::new(&mut self.particle_system.particle_size, 0.1..=5.0)
                        .text("Particle Size"),
                );

                ui.separator();
                ui.heading("Particle Count");

                let max_particles = self.particle_system.max_particles;
                ui.add(
                    egui::Slider::new(&mut self.particle_system.num_particles, 1..=max_particles)
                        .text("Count")
                        .logarithmic(true),
                );

                ui.horizontal(|ui| {
                    if ui.button("10,000").clicked() {
                        self.particle_system.num_particles = 10_000.min(max_particles);
                    }
                    if ui.button("100,000").clicked() {
                        self.particle_system.num_particles = 100_000.min(max_particles);
                    }
                    if ui.button("1,000,000").clicked() {
                        self.particle_system.num_particles = 1_000_000.min(max_particles);
                    }
                });

                ui.separator();
                ui.heading("Display");

                egui::ComboBox::from_label("Color Mode")
                    .selected_text(match self.particle_system.color_mode {
                        0 => "Original",
                        1 => "Velocity",
                        2 => "Position",
                        _ => "Unknown",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.particle_system.color_mode, 0, "Original");
                        ui.selectable_value(&mut self.particle_system.color_mode, 1, "Velocity");
                        ui.selectable_value(&mut self.particle_system.color_mode, 2, "Position");
                    });

                ui.separator();
                ui.heading("Controls");
                ui.label("WASD - Move camera");
                ui.label("Mouse Right - Rotate camera");
                ui.label("Space/Shift - Move up/down");
                ui.label("Mouse Left - Drag particles");
                ui.label("Mouse Scroll - Cursor Distance");
                ui.label("U - Toggle UI");
                ui.label("F11 - Toggle fullscreen");
                ui.label("ESC - Exit");
            });
    }
}

pub struct App {
    instance: wgpu::Instance,
    state: Option<AppState>,
    window: Option<Arc<Window>>,
}

impl App {
    pub fn new() -> Self {
        let instance = egui_wgpu::wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        Self {
            instance,
            state: None,
            window: None,
        }
    }

    async fn set_window(&mut self, window: Window) {
        let window = Arc::new(window);
        let initial_width = 1360;
        let initial_height = 768;

        let _ = window.request_inner_size(PhysicalSize::new(initial_width, initial_height));

        let surface = self
            .instance
            .create_surface(window.clone())
            .expect("Failed to create surface!");

        let state = AppState::new(
            &self.instance,
            surface,
            &window,
            initial_width,
            initial_height,
        )
        .await;

        self.window = Some(window.clone());
        self.state = Some(state);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_title("Particle Simulator"))
            .unwrap();
        pollster::block_on(self.set_window(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        // Let egui process the event first
        if let Some(state) = &mut self.state {
            if let Some(window) = self.window.clone() {
                state.egui_renderer.handle_input(&window, &event);
            }

            match event {
                WindowEvent::KeyboardInput {
                    event: keyboard_input,
                    ..
                } => {
                    state
                        .input_manager
                        .handle_keyboard_input(keyboard_input.clone());

                    if keyboard_input.state == ElementState::Pressed {
                        match keyboard_input.physical_key {
                            winit::keyboard::PhysicalKey::Code(KeyCode::Escape) => {
                                event_loop.exit();
                            }
                            winit::keyboard::PhysicalKey::Code(KeyCode::KeyU) => {
                                state.show_ui = !state.show_ui;
                            }
                            winit::keyboard::PhysicalKey::Code(KeyCode::KeyP) => {
                                state.particle_system.paused = !state.particle_system.paused;
                            }
                            winit::keyboard::PhysicalKey::Code(KeyCode::F11) => {
                                state.fullscreen = !state.fullscreen;
                                let window = self.window.as_ref().unwrap();
                                if state.fullscreen {
                                    window.set_fullscreen(Some(
                                        winit::window::Fullscreen::Borderless(None),
                                    ));
                                } else {
                                    window.set_fullscreen(None);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let delta_value = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                    };

                    state.input_manager.handle_mouse_wheel(delta_value);

                    // If Ctrl is pressed, adjust camera zoom
                    if state.input_manager.is_key_pressed(KeyCode::ControlLeft)
                        || state.input_manager.is_key_pressed(KeyCode::ControlRight)
                    {
                        let mut fov_degrees = state.camera.fov * 180.0 / std::f32::consts::PI;

                        fov_degrees = (fov_degrees - delta_value).clamp(1.0, 180.0);

                        state.camera.fov = fov_degrees * std::f32::consts::PI / 180.0;
                        state.camera.update_view_proj();
                    } else {
                        // Otherwise adjust mouse depth
                        state.particle_system.mouse_depth =
                            state.particle_system.mouse_depth - delta_value * 5.0;
                    }
                }
                WindowEvent::MouseInput {
                    button,
                    state: button_state,
                    ..
                } => {
                    state
                        .input_manager
                        .handle_mouse_button(button, button_state);

                    // Toggle mouse capture for camera rotation
                    if button == MouseButton::Right {
                        let window = self.window.as_ref().unwrap();
                        if button_state == ElementState::Pressed {
                            window.set_cursor_grab(CursorGrabMode::Confined).ok();
                            window.set_cursor_visible(false);
                            state.input_manager.set_mouse_captured(true);
                        } else {
                            window.set_cursor_grab(CursorGrabMode::None).ok();
                            window.set_cursor_visible(true);
                            state.input_manager.set_mouse_captured(false);
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    state
                        .input_manager
                        .handle_mouse_motion(position.x as f32, position.y as f32);
                }
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::RedrawRequested => {
                    state.update();
                    match state.render() {
                        Ok(_) => {}
                        Err(SurfaceError::Lost) => state.resize_surface(
                            state.surface_config.width,
                            state.surface_config.height,
                        ),
                        Err(SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => eprintln!("{:?}", e),
                    }
                    self.window.as_ref().unwrap().request_redraw();
                }
                WindowEvent::Resized(new_size) => {
                    state.resize_surface(new_size.width, new_size.height);
                }
                _ => (),
            }
        }
    }
}
