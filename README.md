# 3D Particle Simulation
This project is built with [`Rust`](https://www.rust-lang.org/), [`Winit`](https://github.com/rust-windowing/winit), [`Wgpu`](https://github.com/gfx-rs/wgpu) and [`Egui`](https://github.com/emilk/egui).

Its heavily inspired by [this project](https://github.com/Im-Rises/particle-simulator-webgl).

I'm making this project with the goal of learning modern graphics programming and the differences between graphics library stacks.

A 2D version of this simulation that uses [`Zig`](https://ziglang.org/) + [`SDL3`](https://github.com/libsdl-org/SDL) + [`Dawn`](https://github.com/google/dawn) / [`Wgpu-Native`](https://github.com/gfx-rs/wgpu-native) + [`ImGui`](https://github.com/ocornut/imgui) can be found [here](https://github.com/lucascompython/particle-simulation-2d).

This project initially used this [template](https://github.com/kaphula/winit-egui-wgpu-template) but I migrated to [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) using the [eframe template](https://github.com/emilk/eframe_template).


## Simulation Methods
The simulation can run on different methods, such as:
- CPU - Works everywhere but has limited performance
- GPU (Compute Shaders) - Only works on native and WebGpu (no WebGl support) but has much better performance

I wanted to add another GPU method, namely Transform Feedback since it is (I think) pretty the most performant method for this simulation that can run on WebGl, but I've found it difficult to implement in `wgpu`. Here is a [discussion](https://github.com/gfx-rs/wgpu/discussions/7601) about it. Still looking into it!

## Build Locally

### Build Release
```bash
./build.sh
# OR
./build.ps1
```

### Native Development
```bash
cargo run
```

### Web Development
```bash
trunk serve
```
And go to [http://127.0.0.1:8080/index.html#dev](http://127.0.0.1:8080/index.html#dev)
The `#dev` is to skip the cache [assets/sw.js](/assets/sw.js) provides.

## TODO:
- Add Web support
- Improve performance, especially startup time
- Add more settings and values to tinker with in the simulation
- ~~Add 2D version, basically rewrite [this](https://github.com/lucascompython/particles).~~ Decided to pursue this with a different stack and in a [separate repository](https://github.com/lucascompython/particle-simulation-2d).
- Make CI work nicely
- Add more color profiles
- Improve binary size
- Update to wgpu 25
