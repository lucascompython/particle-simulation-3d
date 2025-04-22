#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use std::sync::Arc;

    #[cfg(feature = "logs")]
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        renderer: eframe::Renderer::Wgpu,
        // TODO: Check this
        wgpu_options: egui_wgpu::WgpuConfiguration {
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: None, // Use default

            // This is where we customize the device setup:
            wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(egui_wgpu::WgpuSetupCreateNew {
                // Use default instance descriptor (important for web compatibility)
                instance_descriptor: wgpu::InstanceDescriptor::default(),

                // High performance is good for particle simulations
                power_preference: wgpu::PowerPreference::HighPerformance,

                // No custom adapter selector for better web compatibility
                native_adapter_selector: None,

                // THIS is where we configure the device limits:
                device_descriptor: Arc::new(|adapter| {
                    let mut limits = adapter.limits();

                    // Increase storage buffer limits (needed for compute shaders)
                    limits.max_storage_buffers_per_shader_stage =
                        limits.max_storage_buffers_per_shader_stage.max(8);
                    limits.max_storage_buffer_binding_size =
                        limits.max_storage_buffer_binding_size.max(128 << 20); // 128 MB

                    let mut features = wgpu::Features::empty();

                    wgpu::DeviceDescriptor {
                        label: Some("Particle Simulation Device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: limits,
                        memory_hints: wgpu::MemoryHints::default(),
                    }
                }),

                trace_path: None,
            }),

            on_surface_error: Arc::new(|error| {
                eprintln!("Surface error: {:?}", error);
                egui_wgpu::SurfaceErrorAction::RecreateSurface
            }),
        },
        depth_buffer: 0,
        multisampling: 1,
        ..Default::default()
    };
    eframe::run_native(
        "Particle Simulation",
        native_options,
        Box::new(|cc| Ok(Box::new(particle_simulation::ParticleApp::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    #[cfg(feature = "logs")]
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("canvas_id")
            .expect("Failed to find canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(particle_simulation::ParticleApp::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
