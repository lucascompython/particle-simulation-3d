use crate::camera::Camera;
use crate::custom_renderer::UnsafeParticleCallback;
use crate::renderer::ParticleRenderer;

use crate::simulation::compute::ComputeParticleSimulation;
use crate::simulation::cpu::CpuParticleSimulation;
use crate::simulation::transform_feedback::TransformFeedbackSimulation;
use crate::simulation::{ParticleSimulation, SimParams, SimulationMethod};

use glam::Vec3;
use std::collections::HashSet;
use std::time::Instant;

pub struct ParticleApp {
    simulation: Box<dyn ParticleSimulation>,
    renderer: ParticleRenderer,
    camera: Camera,

    // Simulation parameters
    gravity: f32,
    color_mode: u32,
    mouse_force: f32,
    mouse_radius: f32,
    mouse_position: [f32; 3],

    // UI state
    show_ui: bool,
    fps: f32,
    fps_counter: u32,
    fps_timer: f32,
    last_update: Instant,

    current_method: SimulationMethod,
    available_methods: Vec<SimulationMethod>,
    particle_count_input: String,

    // Input tracking
    mouse_pos: (f32, f32),
    mouse_prev_pos: (f32, f32),
    mouse_dragging: bool,
    right_mouse_down: bool,
    keys_down: HashSet<egui::Key>,
    shift_down: bool,
}

impl ParticleApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Get the wgpu render state
        let wgpu_render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("This app requires the wgpu render state");

        let device = &wgpu_render_state.device;

        // Initialize camera
        let size = cc.egui_ctx.screen_rect().size();
        let aspect_ratio = size.x / size.y;
        let camera = Camera::new(device, aspect_ratio);

        // Determine available simulation methods based on capabilities
        let mut available_methods = vec![SimulationMethod::Cpu]; // CPU always available

        // Check if we can use compute shaders (not available in WebGL)
        let has_compute = device.limits().max_compute_workgroup_storage_size > 0;
        if has_compute {
            available_methods.push(SimulationMethod::ComputeShader);
        }

        // Transform feedback is available everywhere
        available_methods.push(SimulationMethod::TransformFeedback);

        // Default to best available method
        let default_method = if has_compute {
            SimulationMethod::ComputeShader
        } else {
            SimulationMethod::TransformFeedback
        };

        // Create simulation based on method
        let initial_particles = 10_000;
        let simulation: Box<dyn ParticleSimulation> = match default_method {
            SimulationMethod::Cpu => {
                Box::new(CpuParticleSimulation::new(device, initial_particles))
            }
            SimulationMethod::ComputeShader => {
                Box::new(ComputeParticleSimulation::new(device, initial_particles))
            }
            SimulationMethod::TransformFeedback => {
                Box::new(TransformFeedbackSimulation::new(device, initial_particles))
            }
        };

        // Create renderer
        let particle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle.wgsl").into()),
        });

        let surface_format = wgpu_render_state.target_format;
        let renderer = ParticleRenderer::new(device, &camera, &surface_format, &particle_shader);

        Self {
            simulation,
            renderer,
            camera,

            gravity: 0.0,
            color_mode: 0,
            mouse_force: 5.0,
            mouse_radius: 10.0,
            mouse_position: [0.0, 0.0, 0.0],

            show_ui: true,
            fps: 0.0,
            fps_counter: 0,
            fps_timer: 0.0,
            last_update: Instant::now(),

            current_method: default_method,
            available_methods,
            particle_count_input: initial_particles.to_string(),

            mouse_pos: (0.0, 0.0),
            mouse_prev_pos: (0.0, 0.0),
            mouse_dragging: false,
            right_mouse_down: false,
            keys_down: HashSet::new(),
            shift_down: false,
        }
    }

    fn change_simulation_method(&mut self, new_method: SimulationMethod, device: &wgpu::Device) {
        if self.current_method == new_method {
            return;
        }

        // Get current count to preserve when switching
        let current_count = self.simulation.get_particle_count();
        let was_paused = self.simulation.is_paused();

        // Create new simulation with the same particle count
        self.simulation = match new_method {
            SimulationMethod::Cpu => Box::new(CpuParticleSimulation::new(device, current_count)),
            SimulationMethod::ComputeShader => {
                Box::new(ComputeParticleSimulation::new(device, current_count))
            }
            SimulationMethod::TransformFeedback => {
                Box::new(TransformFeedbackSimulation::new(device, current_count))
            }
        };

        self.simulation.set_paused(was_paused);
        self.current_method = new_method;
    }

    fn update_simulation(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Calculate delta time
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        // Update FPS counter
        self.fps_counter += 1;
        self.fps_timer += delta_time;
        if self.fps_timer >= 1.0 {
            self.fps = self.fps_counter as f32 / self.fps_timer;
            self.fps_counter = 0;
            self.fps_timer = 0.0;
        }

        // Handle keyboard input for camera movement
        for key in [
            egui::Key::W,
            egui::Key::S,
            egui::Key::A,
            egui::Key::D,
            egui::Key::Space,
        ] {
            if self.keys_down.contains(&key) {
                self.camera
                    .process_keyboard(Some(key), self.shift_down, delta_time);
            }
        }

        if self.shift_down {
            self.camera.process_keyboard(None, true, delta_time);
        }

        // Get wgpu render state for queue access
        if let Some(wgpu_render_state) = frame.wgpu_render_state() {
            let queue = &wgpu_render_state.queue;
            let device = &wgpu_render_state.device;

            // Update camera uniform buffer
            self.camera.update_buffer(queue);

            // Handle mouse position for particle interaction
            if self.mouse_dragging {
                let screen_rect = ctx.screen_rect();
                let (x, y) = self.mouse_pos;

                // Convert screen coordinates to normalized device coordinates
                let ndc_x = (2.0 * x / screen_rect.width()) - 1.0;
                let ndc_y = 1.0 - (2.0 * y / screen_rect.height());

                // Calculate world position using camera
                let camera_forward = self.camera.get_forward();
                let camera_right = self.camera.get_right();
                let camera_up = self.camera.get_up();

                let current_pos = glam::Vec3::new(
                    self.mouse_position[0],
                    self.mouse_position[1],
                    self.mouse_position[2],
                );

                let camera_pos = self.camera.position;
                let to_cursor = current_pos - camera_pos;
                let distance = to_cursor.dot(camera_forward);

                // Calculate the plane at the specified distance from camera
                let plane_center = camera_pos + camera_forward * distance;

                // Scale the NDC coordinates based on the field of view and distance
                let height = 2.0 * distance * (self.camera.fov / 2.0).tan();
                let width = height * self.camera.aspect;

                let world_pos = plane_center
                    + camera_right * (ndc_x * width / 2.0)
                    + camera_up * (ndc_y * height / 2.0);

                self.mouse_position = [world_pos.x, world_pos.y, world_pos.z];
            }

            // Update particle simulation if not paused
            if !self.simulation.is_paused() {
                // Create a command encoder for this frame
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Particle Update Encoder"),
                });

                // Build simulation parameters
                let sim_params = SimParams {
                    delta_time,
                    gravity: self.gravity,
                    color_mode: self.color_mode,
                    mouse_force: self.mouse_force,
                    mouse_radius: self.mouse_radius,
                    mouse_position: self.mouse_position,
                    is_mouse_dragging: if self.mouse_dragging { 1 } else { 0 },
                    damping: 0.99, // Add damping factor
                    _padding1: 0,
                    _padding2: 0,
                };

                // Run the particle simulation using current method
                self.simulation
                    .update(device, queue, &mut encoder, &sim_params);

                // Submit the work
                queue.submit(Some(encoder.finish()));
            }
        }
    }

    fn render_ui(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::Window::new("Particle Simulator")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Statistics");
                ui.label(format!("FPS: {:.1}", self.fps));
                ui.label(format!(
                    "Particles: {}",
                    self.simulation.get_particle_count()
                ));

                ui.separator();
                ui.heading("Simulation");

                let paused = self.simulation.is_paused();
                if ui.button(if paused { "Resume" } else { "Pause" }).clicked() {
                    self.simulation.set_paused(!paused);
                }

                if ui.button("Reset").clicked() {
                    if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                        self.simulation
                            .reset(&wgpu_render_state.device, &wgpu_render_state.queue);
                    }
                }

                let mut clicked_method = None;
                egui::ComboBox::from_label("Method")
                    .selected_text(format!("{:?}", self.current_method))
                    .show_ui(ui, |ui| {
                        for method in &self.available_methods {
                            let text = match method {
                                SimulationMethod::Cpu => "CPU (Compatible Everywhere)",
                                SimulationMethod::ComputeShader => "Compute Shader (Fastest)",
                                SimulationMethod::TransformFeedback => {
                                    "Transform Feedback (WebGL Compatible)"
                                }
                            };

                            if ui
                                .selectable_value(&mut self.current_method, *method, text)
                                .clicked()
                            {
                                clicked_method = Some(*method);
                            }
                        }
                    });

                if let Some(method) = clicked_method {
                    if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                        self.change_simulation_method(method, &wgpu_render_state.device);
                    }
                }

                ui.separator();
                ui.heading("Mouse Interaction");
                ui.label(format!(
                    "Position: ({:.2}, {:.2}, {:.2})",
                    self.mouse_position[0], self.mouse_position[1], self.mouse_position[2]
                ));

                ui.label(format!("Dragging: {}", self.mouse_dragging));
                ui.label(format!("Depth: {:.2}", self.mouse_position[2]));

                ui.add(egui::Slider::new(&mut self.mouse_radius, 1.0..=50.0).text("Radius"));

                ui.add(egui::Slider::new(&mut self.mouse_force, 0.0..=100.0).text("Force"));

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

                // Convert to radians and update camera if changed
                if (fov_degrees * std::f32::consts::PI / 180.0 - self.camera.fov).abs() > 0.001 {
                    self.camera.fov = fov_degrees * std::f32::consts::PI / 180.0;
                    self.camera.update_view_proj();

                    if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                        self.camera.update_buffer(&wgpu_render_state.queue);
                    }
                }

                ui.separator();
                ui.heading("Particle Settings");

                ui.add(egui::Slider::new(&mut self.gravity, 0.0..=5.0).text("Gravity"));

                ui.separator();
                ui.heading("Particle Count");

                ui.horizontal(|ui| {
                    ui.label("Count:");
                    if ui
                        .text_edit_singleline(&mut self.particle_count_input)
                        .lost_focus()
                    {
                        if let Ok(count) = self.particle_count_input.parse::<u32>() {
                            if count > 0 {
                                if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                                    self.simulation.resize_buffer(
                                        &wgpu_render_state.device,
                                        &wgpu_render_state.queue,
                                        count,
                                    );
                                }
                            }
                        }
                    }
                });

                // Quick selection buttons
                ui.horizontal(|ui| {
                    if ui.button("10,000").clicked() {
                        self.particle_count_input = "10000".to_string();
                        if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                            self.simulation.resize_buffer(
                                &wgpu_render_state.device,
                                &wgpu_render_state.queue,
                                10_000,
                            );
                        }
                    }
                    if ui.button("100,000").clicked() {
                        self.particle_count_input = "100000".to_string();
                        if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                            self.simulation.resize_buffer(
                                &wgpu_render_state.device,
                                &wgpu_render_state.queue,
                                100_000,
                            );
                        }
                    }
                    if ui.button("1,000,000").clicked() {
                        self.particle_count_input = "1000000".to_string();
                        if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                            self.simulation.resize_buffer(
                                &wgpu_render_state.device,
                                &wgpu_render_state.queue,
                                1_000_000,
                            );
                        }
                    }
                });

                ui.separator();
                ui.heading("Display");

                egui::ComboBox::from_label("Color Mode")
                    .selected_text(match self.color_mode {
                        0 => "Original",
                        1 => "Velocity",
                        2 => "Position",
                        _ => "Unknown",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.color_mode, 0, "Original");
                        ui.selectable_value(&mut self.color_mode, 1, "Velocity");
                        ui.selectable_value(&mut self.color_mode, 2, "Position");
                    });

                ui.separator();
                ui.heading("Controls");
                ui.label("WASD - Move camera");
                ui.label("Mouse Right - Rotate camera");
                ui.label("Space/Shift - Move up/down");
                ui.label("Mouse Left - Drag particles");
                ui.label("Mouse Scroll - Cursor Distance");
                ui.label("U - Toggle UI");
                ui.label("ESC - Exit");
            });
    }
}

impl eframe::App for ParticleApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Handle keyboard input
        if ctx.input(|i| i.key_pressed(egui::Key::U)) {
            self.show_ui = !self.show_ui;
        }

        // Update key states and modifiers
        ctx.input(|input| {
            // Clear and rebuild the set of keys that are currently down
            self.keys_down.clear();
            for key in egui::Key::ALL {
                if input.key_down(*key) {
                    self.keys_down.insert(*key);
                }
            }

            // Track shift key state
            self.shift_down = input.modifiers.shift;

            // Track mouse position
            self.mouse_prev_pos = self.mouse_pos;
            if let Some(pos) = input.pointer.hover_pos() {
                self.mouse_pos = (pos.x, pos.y);
            }

            // Track mouse dragging for particle interaction
            self.mouse_dragging = input.pointer.primary_down();
            if input.pointer.secondary_down() {
                // Get the actual pointer delta from egui (this is more reliable)
                // ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::None);
                let delta = input.pointer.delta();

                // Only rotate if there's actual movement
                if delta.x != 0.0 || delta.y != 0.0 {
                    self.camera.process_mouse_movement(delta.x, delta.y);
                }
            }

            // Handle scroll for cursor depth adjustment
            if input.raw_scroll_delta.y != 0.0 {
                let scroll_delta = input.raw_scroll_delta.y;

                // Move cursor position along camera forward vector
                let camera_forward = self.camera.get_forward();
                let current_pos = Vec3::new(
                    self.mouse_position[0],
                    self.mouse_position[1],
                    self.mouse_position[2],
                );

                let move_distance = scroll_delta * 0.2; // Adjust sensitivity
                let new_pos = current_pos + camera_forward * move_distance;
                self.mouse_position = [new_pos.x, new_pos.y, new_pos.z];
            }
        });

        // Update simulation state
        self.update_simulation(ctx, frame);

        // Create a central panel to render our 3D content
        egui::CentralPanel::default().show(ctx, |ui| {
            // Get the available space for rendering
            let rect = ui.max_rect();

            // Capture rect size for aspect ratio updates
            let size = rect.size();
            let aspect_ratio = size.x / size.y;
            if (aspect_ratio - self.camera.aspect).abs() > 0.001 {
                self.camera.aspect = aspect_ratio;
                self.camera.update_view_proj();

                if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                    self.camera.update_buffer(&wgpu_render_state.queue);
                }
            }

            let callback_obj = UnsafeParticleCallback {
                render_pipeline_ptr: &self.renderer.render_pipeline as *const _,
                camera_bind_group_ptr: &self.camera.bind_group as *const _,
                particle_buffer_ptr: self.simulation.get_particle_buffer() as *const _,
                num_particles: self.simulation.get_particle_count(),
            };

            let callback = egui_wgpu::Callback::new_paint_callback(rect, callback_obj);
            ui.painter().add(callback);
        });

        // Show UI if enabled
        if self.show_ui {
            self.render_ui(ctx, frame);
        }

        // Request continuous repaints for smooth animation
        ctx.request_repaint();
    }
}
