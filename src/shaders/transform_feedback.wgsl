struct ParticleInput {
    @location(0) position: vec3<f32>,
    @location(1) padding1: f32,
    @location(2) velocity: vec3<f32>,
    @location(3) padding2: f32,
    @location(4) color: vec4<f32>,
};

// Output structure matching the Particle struct layout for TF capture
struct ParticleOutput {
    @location(0) out_position: vec3<f32>,
    @location(1) out_padding1: f32,
    @location(2) out_velocity: vec3<f32>,
    @location(3) out_padding2: f32,
    @location(4) out_color: vec4<f32>,
    // We still need a dummy @builtin(position) for the rasterizer, even if TF is active
    @builtin(position) clip_position: vec4<f32>,
};

// Uniforms (same as compute shader)
struct SimParams {
  // First 16 bytes
  delta_time: f32,
  gravity: f32,
  color_mode: u32,
  mouse_force: f32,

  // Second 16 bytes
  mouse_radius: f32,
  is_mouse_dragging: u32,
  damping: f32,
  _padding1: u32, // Ensure correct padding for std140/std430 alignment

  // Last 16 bytes
  mouse_position: vec3<f32>,
  _padding2: u32, // Ensure correct padding
};

@group(0) @binding(0) var<uniform> params: SimParams;

@vertex
fn vs_main(particle_in: ParticleInput) -> ParticleOutput {

    // --- Simulation Logic (using particle_in) ---
    var new_velocity = particle_in.velocity;
    var new_position = particle_in.position;
    var new_color    = particle_in.color;

    // Apply gravity
    new_velocity.y -= params.gravity * params.delta_time;

    // Apply mouse force
    if (params.is_mouse_dragging > 0u) {
        let dir = params.mouse_position - new_position;
        let dist = length(dir);

        if (dist < params.mouse_radius * 2.0) {
            // Make the force stronger overall and more dramatic for closer particles
            let force_factor = pow(1.0 - dist / (params.mouse_radius * 2.0), 2.0) * 2.0;
            let force = normalize(dir) * params.mouse_force * force_factor;
            new_velocity += force * params.delta_time;
        }
    }

    // Update position
    new_position += new_velocity * params.delta_time;

    // Handle boundaries
    let bounds = 500.0;
    if (new_position.x < -bounds) {
        new_position.x = -bounds;
        new_velocity.x = abs(new_velocity.x) * 0.5;
    } else if (new_position.x > bounds) {
        new_position.x = bounds;
        new_velocity.x = -abs(new_velocity.x) * 0.5;
    }
    // ... (repeat for y and z boundaries) ...
    if (new_position.y < -bounds) {
        new_position.y = -bounds;
        new_velocity.y = abs(new_velocity.y) * 0.5;
    } else if (new_position.y > bounds) {
        new_position.y = bounds;
        new_velocity.y = -abs(new_velocity.y) * 0.5;
    }
    if (new_position.z < -bounds) {
        new_position.z = -bounds;
        new_velocity.z = abs(new_velocity.z) * 0.5;
    } else if (new_position.z > bounds) {
        new_position.z = bounds;
        new_velocity.z = -abs(new_velocity.z) * 0.5;
    }


    // Apply damping
    new_velocity *= params.damping;

    // Update color based on mode
    if (params.color_mode == 1u) {
        // Velocity-based coloring
        let speed = length(new_velocity);
        let norm_speed = min(speed / 5.0, 1.0);
        new_color = vec4<f32>(norm_speed, 0.5 - norm_speed * 0.5, 1.0 - norm_speed, 1.0);
    } else if (params.color_mode == 2u) {
        // Position-based coloring
        let norm_pos = (new_position / bounds + vec3<f32>(1.0)) * 0.5;
        new_color = vec4<f32>(norm_pos.x, norm_pos.y, norm_pos.z, 1.0);
    }
    // else: keep original color (already in new_color)

    // --- Output the NEW state ---
    var output: ParticleOutput;
    output.out_position = new_position;
    output.out_padding1 = 0.0; // Ensure padding is written
    output.out_velocity = new_velocity;
    output.out_padding2 = 0.0; // Ensure padding is written
    output.out_color = new_color;
    output.clip_position = vec4(0.0, 0.0, 0.0, 1.0); // Dummy position, not used for rendering here

    return output;
}

@fragment
fn fs_dummy_main() {
    
}