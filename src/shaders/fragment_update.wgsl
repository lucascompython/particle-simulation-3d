@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Create a full-screen triangle strip (quad)
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0)
    );

    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

// Simulation parameters
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
};

// Particle structure (matches the one in compute.wgsl)
struct Particle {
    position: vec3<f32>,
    padding1: f32,
    velocity: vec3<f32>,
    padding2: f32,
    color: vec4<f32>,
};

// Binding resources
@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> params: SimParams;
@group(0) @binding(3) var<storage, read_write> output_particles: array<Particle>;

// Fragment shader that updates particle physics
@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // Calculate the particle index from fragment coordinates
    let texture_size = vec2<f32>(params.texture_width, params.texture_height);
    let coord = vec2<i32>(frag_coord.xy);
    let index = u32(coord.y) * u32(params.texture_width) + u32(coord.x);

    // Don't process particles beyond the count
    if (index >= params.num_particles) {
        return vec4<f32>(0.0);
    }

    // Get UV coordinates
    let uv = vec2<f32>(frag_coord.xy) / texture_size;

    // Sample the source texture to get current particle state
    let particle_pos_vel = textureLoad(source_texture, coord, 0);

    // Extract position and velocity
    var position = vec3<f32>(particle_pos_vel.xyz);
    var velocity = vec3<f32>(particle_pos_vel.aaa); // This is simplified, would need more textures

    // Apply gravity
    velocity.y -= params.gravity * params.delta_time;

    // Apply mouse force if dragging
    if (params.is_mouse_dragging > 0u) {
        let mouse_pos_3d = vec3<f32>(
            params.mouse_position_x,
            params.mouse_position_y,
            params.mouse_position_z
        );

        let dir = mouse_pos_3d - position;
        let dist = length(dir);

        if (dist < params.mouse_radius * 2.0) {
            let force_factor = pow(1.0 - dist / (params.mouse_radius * 2.0), 2.0) * 2.0;
            let force = normalize(dir) * params.mouse_force * force_factor;
            velocity += force * params.delta_time;
        }
    }

    // Update position
    position += velocity * params.delta_time;

    // Boundary conditions
    let bounds = 500.0;
    if (position.x < -bounds) {
        position.x = -bounds;
        velocity.x = abs(velocity.x) * 0.5;
    } else if (position.x > bounds) {
        position.x = bounds;
        velocity.x = -abs(velocity.x) * 0.5;
    }

    if (position.y < -bounds) {
        position.y = -bounds;
        velocity.y = abs(velocity.y) * 0.5;
    } else if (position.y > bounds) {
        position.y = bounds;
        velocity.y = -abs(velocity.y) * 0.5;
    }

    if (position.z < -bounds) {
        position.z = -bounds;
        velocity.z = abs(velocity.z) * 0.5;
    } else if (position.z > bounds) {
        position.z = bounds;
        velocity.z = -abs(velocity.z) * 0.5;
    }

    // Apply damping
    velocity *= 0.99;

    // Also update the output buffer for rendering
    output_particles[index].position = position;
    output_particles[index].velocity = velocity;

    // Update color based on mode (same as compute shader)
    if (params.color_mode == 1u) {
        // Velocity-based coloring
        let speed = length(velocity);
        let norm_speed = min(speed / 5.0, 1.0);
        output_particles[index].color = vec4<f32>(norm_speed, 0.5 - norm_speed * 0.5, 1.0 - norm_speed, 1.0);
    } else if (params.color_mode == 2u) {
        // Position-based coloring
        let norm_pos = (position / bounds + 1.0) * 0.5;
        output_particles[index].color = vec4<f32>(norm_pos.x, norm_pos.y, norm_pos.z, 1.0);
    }

    // Return the updated particle data to be stored in the target texture
    return vec4<f32>(position, velocity.x); // Packing velocity into the alpha for simplicity
}
