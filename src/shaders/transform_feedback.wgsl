struct Particle {
    position: vec3<f32>,
    padding1: f32,
    velocity: vec3<f32>,
    padding2: f32,
    color: vec4<f32>,
};

struct SimParams {
    delta_time: f32,
    gravity: f32,
    color_mode: u32,
    mouse_force: f32,
    
    mouse_radius: f32,
    is_mouse_dragging: u32,
    damping: f32,
    _padding1: u32,
    
    mouse_position: vec3<f32>,
    _padding2: u32,
};

@group(0) @binding(0) var<uniform> params: SimParams;

struct VertexOutput {
    // Dummy position for rasterizer (not used)
    @builtin(position) clip_position: vec4<f32>,
    
    // Transformed attributes to be captured
    @location(0) new_position: vec3<f32>,
    @location(1) new_padding1: f32,
    @location(2) new_velocity: vec3<f32>,
    @location(3) new_padding2: f32,
    @location(4) new_color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) padding1: f32,
    @location(2) velocity: vec3<f32>,
    @location(3) padding2: f32,
    @location(4) color: vec4<f32>,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var output: VertexOutput;
    
    // --- Physics calculations to match compute/CPU methods ---
    var new_position = position;
    var new_velocity = velocity;
    var new_color = color;
    
    // Apply gravity (straight down)
    new_velocity.y -= params.gravity * params.delta_time;
    
    // Apply mouse force if dragging
    // if (params.is_mouse_dragging > 0u) {
        let dir = params.mouse_position - new_position;
        let dist = length(dir);
        
        if (dist < params.mouse_radius * 2.0) {
            let force_factor = pow(1.0 - dist / (params.mouse_radius * 2.0), 2.0) * 2.0;
            let force = normalize(dir) * params.mouse_force * force_factor;
            new_velocity += force * params.delta_time;
        }
    // }
    
    // Update position based on velocity
    new_position += new_velocity * params.delta_time;
    
    // Boundary checks
    let bounds = 500.0;
    
    if (new_position.x < -bounds) {
        new_position.x = -bounds;
        new_velocity.x = abs(new_velocity.x) * 0.5;
    } else if (new_position.x > bounds) {
        new_position.x = bounds;
        new_velocity.x = -abs(new_velocity.x) * 0.5;
    }
    
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
    
    // Set output with updated particle data
    output.new_position = new_position;
    output.new_padding1 = padding1;
    output.new_velocity = new_velocity;
    output.new_padding2 = padding2;
    output.new_color = new_color;
    
    // Dummy position for rasterizer - not important since we're not rendering
    output.clip_position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    
    return output;
}

@fragment
fn fs_dummy() {
}