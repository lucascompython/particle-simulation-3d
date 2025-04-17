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
  mouse_position: vec3<f32>,
  is_mouse_dragging: u32,
  damping: f32,
};

@group(0) @binding(0)
var<storage, read> particles_in: array<Particle>;

@group(0) @binding(1)
var<uniform> params: SimParams;

@group(0) @binding(2)
var<storage, read_write> particles_out: array<Particle>;

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
  var output: VertexOutput;

  // Get current particle
  var particle = particles_in[vertex_index];

  // Apply gravity
  particle.velocity.y -= params.gravity * params.delta_time;

  // Apply mouse force (same as compute shader)
  if (params.is_mouse_dragging > 0u) {
    let dir = params.mouse_position - particle.position;
    let dist = length(dir);

    if (dist < params.mouse_radius * 2.0) {
      let force_factor = pow(1.0 - dist / (params.mouse_radius * 2.0), 2.0) * 2.0;
      let force = normalize(dir) * params.mouse_force * force_factor;
      particle.velocity += force * params.delta_time;
    }
  }

  // Update position
  particle.position += particle.velocity * params.delta_time;

  // Handle boundaries
  let bounds = 500.0;

  if (particle.position.x < -bounds) {
    particle.position.x = -bounds;
    particle.velocity.x = abs(particle.velocity.x) * 0.5;
  } else if (particle.position.x > bounds) {
    particle.position.x = bounds;
    particle.velocity.x = -abs(particle.velocity.x) * 0.5;
  }

  if (particle.position.y < -bounds) {
    particle.position.y = -bounds;
    particle.velocity.y = abs(particle.velocity.y) * 0.5;
  } else if (particle.position.y > bounds) {
    particle.position.y = bounds;
    particle.velocity.y = -abs(particle.velocity.y) * 0.5;
  }
  
  if (particle.position.z < -bounds) {
    particle.position.z = -bounds;
    particle.velocity.z = abs(particle.velocity.z) * 0.5;
  } else if (particle.position.z > bounds) {
    particle.position.z = bounds;
    particle.velocity.z = -abs(particle.velocity.z) * 0.5;
  }

  // Apply damping
  particle.velocity *= params.damping;

  // Update color based on mode (same logic as compute shader)
  if (params.color_mode == 1u) {
    // Velocity-based coloring
    let speed = length(particle.velocity);
    let norm_speed = min(speed / 5.0, 1.0);
    particle.color = vec4<f32>(norm_speed, 0.5 - norm_speed * 0.5, 1.0 - norm_speed, 1.0);
  } else if (params.color_mode == 2u) {
    // Position-based coloring
    let norm_pos = (particle.position / bounds + 1.0) * 0.5;
    particle.color = vec4<f32>(norm_pos.x, norm_pos.y, norm_pos.z, 1.0);
  }

  // Write updated particle to output buffer
  particles_out[vertex_index] = particle;

  // We don't actually render anything in this pass
  output.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
  return output;
}
