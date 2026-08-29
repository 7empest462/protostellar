# Protostellar

**Protostellar** is a high-performance, visually stunning Solar System Formation Simulator built in Rust using the Bevy Engine. It simulates the gravitational accretion of a protoplanetary disk, allowing you to watch as thousands of particles organically form into a central star, terrestrial planets, gas giants, and intricate moon systems over millions of simulated years.

## Features

- **N-Body Gravity Simulation:** Uses a high-performance Barnes-Hut octree to compute gravity and collisions for 50,000+ particles simultaneously.
- **Organic Accretion:** Particles collide and merge based on realistic accretion physics, building up planets and capturing moons via aerodynamic gas drag.
- **Procedural Volumetrics:** Includes a time-evolving protoplanetary gas cloud shader that dissipates over time, and procedural planet surface shaders that adapt to surface temperature and mass (Terrestrial, Gas Giant, Ice Giant).
- **Time-Warp Physics:** Smoothly simulates physics at up to 1,000,000x real-time speed, balancing discrete integration steps to maintain orbital stability over millions of years.

## Getting Started

### Prerequisites
- **Rust Toolchain:** You need Rust installed. Get it from [rustup.rs](https://rustup.rs/).
- **GPU:** A modern GPU with Vulkan/Metal/DirectX 12 support (specifically supporting WebGPU compute standards).

### Running the Simulator

Clone the repository and run it in release mode for optimal performance:

```bash
git clone git@github.com:7empest462/protostellar.git
cd protostellar
cargo run --release
```

> **Note:** Always run in `--release` mode! The Barnes-Hut simulation requires compiler optimizations to handle 50,000 particles at 60 FPS.

## Controls

### Navigation
- **W, A, S, D:** Pan the camera around the solar system.
- **Mouse Scroll:** Zoom in and out (exponentially smoothed).
- **Right Click & Drag:** Orbit the camera around your current focal point.

### Interaction
- **Left Click:** Select a celestial body (Star, Planet, or Moon) to lock the camera focus onto it.
- **Tab:** Cycle through all major celestial bodies in the system, sorted by mass (from the star down to the smallest moons).
- **F:** Refocus the camera on your currently selected body.
- **R / Escape:** Reset camera focus to the center of the solar system.
- **Spacebar:** Pause / Resume the physics simulation.
- **Left / Right Arrows:** Decrease or increase the time-warp simulation speed.

## Architecture

Protostellar uses Bevy's ECS (Entity Component System) architecture. The simulation runs on a decoupled fixed-timestep physics loop while the renderer interpolates positions for smooth 120 FPS visuals.

- **`src/simulation/`**: Core physics, Barnes-Hut tree generation, gas drag, and accretion logic.
- **`src/rendering/`**: Procedural wgsl shaders, dynamic instancing of 50,000 particles, and camera smoothing.
- **`src/game/`**: UI overlay, player interaction, and simulation controls.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
