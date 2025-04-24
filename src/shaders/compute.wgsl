struct Particle {
  position: vec3<f32>,
  padding1: f32,
  velocity: vec3<f32>,
  padding2: f32,
  color: vec4<f32>,
  initial_color: vec4<f32>,
};

struct SimParams {
  delta_time: f32,
  gravity: f32,
  color_mode: u32,
  mouse_force: f32,

  mouse_radius: f32,
  is_mouse_dragging: u32,
  damping: f32,
  max_dist_for_color: f32,

  mouse_position: vec3<f32>,
  _padding2: u32,
};

@group(0) @binding(0)
var<storage, read_write> particles: array<Particle>;

@group(0) @binding(1)
var<uniform> params: SimParams;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    // Early return if we're out of bounds
    if index >= arrayLength(&particles) {
        return;
    }

    // Cache frequently used values for better performance
    let delta_time = params.delta_time;
    let gravity = params.gravity;
    let damping = params.damping;
    let max_dist = params.max_dist_for_color;


    var position = particles[index].position;
    var velocity = particles[index].velocity;
    let initial_color = particles[index].initial_color;
    var current_color = particles[index].color;

    // Apply gravity
    velocity.y -= gravity * delta_time;

    // Apply mouse force - only if needed
    if params.is_mouse_dragging > 0u {
        let dir = params.mouse_position - position;
        let dist = length(dir);

        if dist < params.mouse_radius * 2.0 {
        // More efficient force calculation
            let normalized_dist = clamp(dist / (params.mouse_radius * 2.0), 0.0, 1.0);
            let force_factor = (1.0 - normalized_dist) * (1.0 - normalized_dist) * 2.0;
            velocity += normalize(dir) * params.mouse_force * force_factor * delta_time;
        }
    }

    // Update position
    position += velocity * delta_time;

    // Apply damping
    velocity *= damping;

    switch params.color_mode {
        case 0u: {
                current_color = initial_color;
        }
        case 1u: {
                let speed = length(velocity);
                let norm_speed = clamp(speed / 5.0, 0.0, 1.0); // Use clamp for safety
                current_color = vec4<f32>(norm_speed, 0.5 - norm_speed * 0.5, 1.0 - norm_speed, 1.0);
        }
        case 2u: {
            let dist_from_origin = length(position);
            // Normalize distance using max_dist, clamp to [0, 1]
            let norm_dist = clamp(dist_from_origin / max(max_dist, 0.01), 0.0, 1.0);
            // Example coloring: blue near origin, red far away
            current_color = vec4<f32>(norm_dist, 0.0, 1.0 - norm_dist, 1.0);
        }
        default: {
            current_color = initial_color;
        }
    }

    // Write back particle data once
    particles[index].position = position;
    particles[index].velocity = velocity;
    particles[index].color = current_color;
}
