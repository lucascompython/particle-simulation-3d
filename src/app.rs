use crate::camera::Camera;
use crate::custom_renderer::ClonedParticleCallback;
use crate::renderer::ParticleRenderer;

use crate::simulation::compute::ComputeParticleSimulation;
use crate::simulation::cpu::CpuParticleSimulation;
use crate::simulation::{ParticleSimulation, SimParams, SimulationMethod, SphereGeneration};

use egui::epaint::text::{FontInsert, InsertFontFamily};
use glam::Vec3;
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

pub struct ParticleApp {
    simulation: Box<dyn ParticleSimulation>,
    surface_format: wgpu::TextureFormat,
    renderer: ParticleRenderer,
    camera: Camera,

    // Simulation parameters
    gravity: f32,
    color_mode: u32,
    mouse_force: f32,
    mouse_radius: f32,
    mouse_position: [f32; 3],
    max_dist_for_color: f32,

    // UI state
    show_ui: bool,
    fps: f32,
    fps_counter: u32,
    fps_timer: f32,
    last_update: Instant,
    simulation_update_time: f32,

    current_method: SimulationMethod,
    available_methods: Vec<SimulationMethod>,
    ui_particle_count: u32,
    // TODO: see if its possible to  remove the ui specific variable
    generation_mode: SphereGeneration,
    ui_generation_mode: SphereGeneration,

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
        // TODO: see why on release mode the font is not loading properly on some texts
        cc.egui_ctx.add_font(FontInsert::new(
            "Ubuntu Light",
            egui::FontData::from_static(include_bytes!("../assets/Ubuntu-Light.ttf")),
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                },
            ],
        ));

        // Get the wgpu render state
        let wgpu_render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("This app requires the wgpu render state");

        let device = &wgpu_render_state.device;

        // Initialize camera
        let size = cc.egui_ctx.content_rect().size();
        let aspect_ratio = size.x / size.y;
        let camera = Camera::new(device, aspect_ratio);

        // Determine available simulation methods based on capabilities
        let mut available_methods = vec![SimulationMethod::Cpu]; // CPU always available

        // Check if we can use compute shaders (not available in WebGL)
        let has_compute = device.limits().max_compute_workgroup_storage_size > 0;
        if has_compute {
            available_methods.push(SimulationMethod::ComputeShader);
        }

        // Default to best available method
        let default_method = if has_compute {
            SimulationMethod::ComputeShader
        } else {
            SimulationMethod::Cpu
        };

        let surface_format = wgpu_render_state.target_format;
        let initial_generation_mode = SphereGeneration::Hollow;

        let initial_particles;
        let simulation: Box<dyn ParticleSimulation> = match default_method {
            SimulationMethod::Cpu => {
                initial_particles = 100_000;
                Box::new(CpuParticleSimulation::new(
                    device,
                    initial_particles,
                    surface_format,
                    initial_generation_mode,
                ))
            }
            SimulationMethod::ComputeShader => {
                initial_particles = 1_000_000;
                Box::new(ComputeParticleSimulation::new(
                    device,
                    initial_particles,
                    surface_format,
                    initial_generation_mode,
                ))
            }
        };

        let particle_shader = unsafe {
            device.create_shader_module_trusted(
                wgpu::include_wgsl!("shaders/particle.wgsl"),
                wgpu::ShaderRuntimeChecks::unchecked(),
            )
        };

        let surface_format = wgpu_render_state.target_format;
        let renderer = ParticleRenderer::new(device, &camera, &surface_format, &particle_shader);

        Self {
            simulation,
            surface_format,
            renderer,
            camera,

            gravity: 0.0,
            color_mode: 0,
            mouse_force: 5.0,
            mouse_radius: 10.0,
            mouse_position: [0.0, 0.0, 48.0],
            max_dist_for_color: 50.0,

            show_ui: true,
            fps: 0.0,
            fps_counter: 0,
            fps_timer: 0.0,
            last_update: Instant::now(),
            simulation_update_time: 0.0,

            current_method: default_method,
            available_methods,
            ui_particle_count: initial_particles,
            generation_mode: initial_generation_mode,
            ui_generation_mode: initial_generation_mode,

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
            SimulationMethod::Cpu => Box::new(CpuParticleSimulation::new(
                device,
                current_count,
                self.surface_format,
                self.generation_mode,
            )),
            SimulationMethod::ComputeShader => Box::new(ComputeParticleSimulation::new(
                device,
                current_count,
                self.surface_format,
                self.generation_mode,
            )),
        };

        self.simulation.set_paused(was_paused);
        self.current_method = new_method;
        self.ui_particle_count = current_count;
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
                let screen_rect = ctx.content_rect();
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
                    max_dist_for_color: self.max_dist_for_color,
                    _padding2: 0,
                };

                let update_start = Instant::now();

                // Run the particle simulation using current method
                self.simulation
                    .update(device, queue, &mut encoder, &sim_params);

                let update_time_ms = update_start.elapsed().as_secs_f32() * 1000.0;
                const ALPHA: f32 = 0.1;

                // Submit the work
                queue.submit(Some(encoder.finish()));
                self.simulation_update_time =
                    (1.0 - ALPHA) * self.simulation_update_time + ALPHA * update_time_ms;
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
                    "Particles update time: {:.4} ms",
                    self.simulation_update_time
                ));

                ui.separator();
                ui.heading("Simulation");

                ui.horizontal(|ui| {
                    if ui.button("Reset").clicked()
                        && let Some(wgpu_render_state) = frame.wgpu_render_state()
                    {
                        self.simulation.reset(
                            &wgpu_render_state.device,
                            &wgpu_render_state.queue,
                            self.generation_mode,
                        );
                    }

                    let paused = self.simulation.is_paused();
                    if ui.button(if paused { "Resume" } else { "Pause" }).clicked() {
                        self.simulation.set_paused(!paused);
                    }
                });

                let mut clicked_method = None;
                egui::ComboBox::from_label("Method")
                    .selected_text(format!("{:?}", self.current_method))
                    .show_ui(ui, |ui| {
                        for method in &self.available_methods {
                            let text = match method {
                                SimulationMethod::Cpu => "CPU (Compatible Everywhere)",
                                SimulationMethod::ComputeShader => "Compute Shader (Fastest)",
                            };
                            if ui
                                .selectable_label(self.current_method == *method, text)
                                .clicked()
                                && self.current_method != *method
                            {
                                clicked_method = Some(*method);
                            }
                        }
                    });

                if let Some(method) = clicked_method
                    && let Some(wgpu_render_state) = frame.wgpu_render_state()
                {
                    self.change_simulation_method(method, &wgpu_render_state.device);
                }

                ui.separator();
                ui.heading("Generation");
                let mut generation_mode_changed = false;
                ui.horizontal(|ui| {
                    generation_mode_changed |= ui
                        .radio_value(
                            &mut self.ui_generation_mode,
                            SphereGeneration::Hollow,
                            "Hollow Sphere",
                        )
                        .changed();
                    generation_mode_changed |= ui
                        .radio_value(
                            &mut self.ui_generation_mode,
                            SphereGeneration::Filled,
                            "Filled Sphere",
                        )
                        .changed();
                });

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

                let mut particle_count_changed = false; // Flag to trigger resize later

                ui.horizontal(|ui| {
                    ui.label("Count:");
                    // Use DragValue bound to the u32 field
                    let drag_response = ui.add(
                        egui::DragValue::new(&mut self.ui_particle_count).speed(100.0), // Adjust speed as needed (particles per point dragged)
                                                                                        // .suffix(" particles") // Optional suffix
                    );

                    // Check if the DragValue was changed by the user
                    if drag_response.changed() {
                        particle_count_changed = true;
                    }
                });

                // Quick selection buttons
                ui.horizontal(|ui| {
                    let mut set_count = |count: u32| {
                        if self.ui_particle_count != count {
                            self.ui_particle_count = count;
                            particle_count_changed = true; // Signal that resize is needed
                        }
                    };

                    if ui.button("10,000").clicked() {
                        set_count(10_000);
                    }
                    if ui.button("100,000").clicked() {
                        set_count(100_000);
                    }
                    if ui.button("1,000,000").clicked() {
                        set_count(1_000_000);
                    }
                });

                // Apply resize if the count changed via DragValue or buttons
                if particle_count_changed || generation_mode_changed {
                    let count_to_set = self.ui_particle_count.max(1);
                    self.ui_particle_count = count_to_set;
                    self.generation_mode = self.ui_generation_mode;

                    if let Some(wgpu_render_state) = frame.wgpu_render_state() {
                        self.simulation.resize_buffer(
                            &wgpu_render_state.device,
                            &wgpu_render_state.queue,
                            count_to_set,
                            self.generation_mode,
                        );
                    }
                }
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
            });
    }
}

impl eframe::App for ParticleApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::U)) {
            self.show_ui = !self.show_ui;
        }

        // TODO: rethink keyboard input handling
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
                // TODO: Check this
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

            // TODO: See about making this reference counted
            let callback_obj = ClonedParticleCallback {
                render_pipeline: self.renderer.render_pipeline.clone(),
                camera_bind_group: self.camera.bind_group.clone(),
                particle_buffer: self.simulation.get_particle_buffer().clone(),
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
