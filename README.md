# 3D Particle Simulation
This project uses winit, wgpu and egui and is heavily inspired by [this project](https://github.com/Im-Rises/particle-simulator-webgl).

This project also used this [template](https://github.com/kaphula/winit-egui-wgpu-template)

## TODO:
- Add Web support
- Improve performance, especially startup time
- Add more settings and values to tinker with in the simulation
- Add 2D version, rewrite [this](https://github.com/lucascompython/particles) in wgpu
- Add more color profiles
- Improve wasm bundle size
- Update to wgpu 25

## Web Locally
```bash
trunk serve
```
And go to [http://127.0.0.1:8080/index.html#dev](http://127.0.0.1:8080/index.html#dev)   
The `#dev` is to skip the cache [assets/sw.js](/assets/sw.js) provides.
