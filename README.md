# PROTOSTELLAR 🪐✨

**Protostellar** is a high-performance, visually stunning Solar System and Exoplanet Formation Simulator built in Rust using the [Bevy Engine](https://bevyengine.org/). It combines rigorous astrophysical modeling with real-time procedural rendering, allowing you to watch thousands of particles organically coalesce into stars, terrestrial worlds, ringed gas giants, and intricate moon systems over millions of simulated years.

---

## 🌟 Key Features & Astrophysical Engine

### 1. 🌌 N-Body Gravitational Dynamics & Symplectic Leapfrog Integration
- **High-Performance Physics**: Simulates 50,000+ particles and active celestial bodies simultaneously using multi-threaded spatial partitioning.
- **Symplectic Leapfrog Integrator**: Guarantees long-term orbital energy and angular momentum conservation across millions of years, even at extreme time-warps up to $10,000\text{x}$.
- **Bondi-Hoyle & Runaway Jovian Accretion**: Models gravitational capture radii, Hill sphere sweeping, and exponential gas envelope accretion that transitions rocky cores into massive gas giants and stars.

### 2. 🕳️ General Relativistic Gravitational Lensing & Spacetime Warping
- **Ray-Deflection Optics**: Accurately computes Einstein deflection angles $\alpha(\theta) = \frac{\theta_E^2}{\theta}$ for massive black holes, bending background stars, cosmic web filaments, and distant nebulae into genuine **Einstein rings and gravitational arcs**.
- **Photon Sphere Caustic Rings**: Renders the razor-sharp caustic ring at $r \approx 1.5 R_s$ where orbiting photons escape, complete with relativistic Doppler beaming asymmetry.
- **Kerr Spacetime Geodesic Funnels**: Visualizes 12 spiraling geodesic field lines tracing frame-dragging and the spatial metric curvature funneling into the singularity.
- **Dynamic Cocoon Blowout Contraction**: When radiation pressure blows away the $60\text{ AU}$ Little Red Dot cocoon, the entire gravitational lens smoothly contracts down to $2.5\text{ AU}$ around the naked event horizon.

### 3. 🌌 Scenario-Specific Procedural Celestial Skybox
- **Camera-Anchored 1,000,000 AU Celestial Sphere**: Zero depth clipping and zero artificial parallax at any zoom scale ($0.005\text{ AU}$ to $250,000\text{ AU}$).
- **Modern Milky Way & Star Clusters** *(Standard Scenarios)*:
  - Tilted galactic coordinate frame ($60.2^\circ$) with an incandescent golden-amber Sagittarius A* galactic core.
  - Fractal **Great Rift** dark molecular absorption lanes and Bok globules.
  - Multi-spectral starfield spanning authentic Morgan-Keenan spectral classes ($O/B$ blue supergiants, $A/F$ white stars, $G$ yellow suns, $K$ orange giants, and $M$ red dwarfs) with sub-pixel twinkle and diffraction spikes.
  - Open star clusters (Pleiades with electric-blue reflection nebulae), high-halo globular clusters, and $H\alpha / [O\text{ III}]$ emission nebulae.
- **Early Universe High-Redshift Cosmic Web** *(Little Red Dot, $z \sim 8.5$)*:
  - Primeval cosmic dawn environment 600 million years after the Big Bang.
  - 3D cellular intergalactic cosmic web filaments glowing with cosmologically redshifted Lyman-$\alpha$ emission (ruby-crimson, amber-gold, and near-infrared hues).
  - Epoch of Reionization (EoR) Strömgren ionization bubbles with glowing shock rims.
  - Pristine Population III hypergiant starburst knots and distant ruby mini-quasars.

### 4. 🪐 Dynamic Roche Disruption & Planetary Ring Spawning
- **Fluid Roche Limit Shredding**: When a moon, comet, or planetesimal ventures inside a primary's fluid Roche limit $d_{\text{Roche}} \approx 2.44 R_p \left(\frac{\rho_p}{\rho_s}\right)^{1/3}$, tidal forces overcome self-gravity, stretching the body into an ellipsoid before shattering it.
- **Keplerian Spiral Debris Streams**: Renders expanding, 48-fragment Keplerian debris streamers wrapping around the planet's equatorial plane, simulating the dynamic transition from fragmented rubble into a circularized ring plane over time.
- **Composition-Dependent Ring Albedo**: Rings dynamically adopt the color and optical depth of the disrupted body:
  - Brilliant reflective silver-white (`ice >= 70%`, Saturn-like).
  - Warm sand-cream tones (`35% - 70%` ice/dust mixture).
  - Dark charcoal / anthracite (`< 35%` ice, Uranus/Jupiter-like silicate rings).
- **Interactive Experiment**: Trigger on demand via the Planet Builder (`[P]` $\to$ `[ 💥 Insert Sub-Roche Moon ]`).

### 5. 💨 Atmospheric Photoevaporation & Cometary Outflow Tails
- **Extreme UV (EUV) Hydrodynamic Escape**: Close-in worlds ($a < 0.25\text{ AU}$, e.g. Hot Jupiter `[F4]`, TRAPPIST-1b/c `[F2]`) absorb intense high-energy radiation, driving supersonic Parker-type hydrodynamic winds.
- **Energy-Limited Mass Stripping**: Computes mass loss rates $\dot{M}_{\text{loss}} \approx \frac{\eta \pi R_p^3 F_{\text{EUV}}}{G M_p}$, stripping volatile hydrogen/helium envelopes down to bare rocky cores over geological time (the "Hot Neptune Desert").
- **3D Anti-Stellar Cometary Tails**:
  - Central supersonic ion core spine extending $0.25 - 6.0\text{ AU}$ away from the host star with orbital aberration tilt.
  - Parabolic expanding cometary bow shock sheath with 4 flaring azimuthal streamer ribs.
  - Traveling ionization knot pulses flowing down the tail.
  - Composition-dependent ionization glow: electric cyan for Hydrogen/Helium envelopes vs warm incandescent amber for evaporated mineral/sodium vapor.

### 6. 🌍 Planetary Geology, Atmospheres & Cometary Water Delivery
- **Thermal Evolution & Magma Oceans**: Cooling crust plates drift over convective, pulsating lava rifts on molten protoplanets ($T > 600\text{ K}$).
- **Core Differentiation & Geodynamo Shielding**: Iron cores differentiate to generate magnetic fields that shield volatile atmospheres from solar wind hydrodynamic stripping.
- **Late Heavy Bombardment (`[G]`)**: Nice-model $2:1$ resonance migration perturbs outer icy comets into inner orbits. Real-time water delivery builds liquid blue oceans and cloud decks on Proto-Earth.
- **Super-Earths & Gas Giant Variety Engine**: Procedural biomes (cratons, alpine ranges, abyssal trenches, rain forests) and mass-tiered Jovian palettes (amber, teal, sapphire, royal purple, maroon brown dwarfs) with anticyclonic storm vortices.
- **Procedural 3D Irregular Asteroids**: Real-time non-spherical mesh generation for potato chondrites, contact binary dumbbells, and spinning-top rubble piles.

---

## 🚀 Sandbox Scenarios & Presets

Switch between multi-system presets instantly via the top HUD bar or function keys (`F1`–`F6`):

| Preset | Key | Description |
| :--- | :---: | :--- |
| **Hayashi Solar Nebula** | `[F1]` | Canonical 4.5 Gyr Minimum Mass Solar Nebula (MMSN) with central protostar and 8–10 seeded protoplanetary niches. |
| **TRAPPIST-1 System** | `[F2]` | Ultracool M-dwarf with 7 resonant Earth-sized worlds in a compact Laplace chain (3 in the liquid water habitable zone). |
| **Kepler-16 Circumbinary** | `[F3]` | "Tatooine" K/M-dwarf binary pair orbited by a Saturn-mass circumbinary giant with a habitable exomoon and outer ocean world. |
| **Hot Jupiter Migration** | `[F4]` | Massive $1.4\text{ M}_{\text{Jup}}$ gas giant undergoing Type II disk torque inward migration from $5.2\text{ AU}$ down to $0.045\text{ AU}$. |
| **Rogue Planet Flyby** | `[F5]` | Unbound $3.5\text{ M}_{\text{Jup}}$ interstellar interloper screaming through the system at $38\text{ km/s}$, scattering comets and tilting orbits. |
| **JWST Little Red Dot** | `[F6]` | Cosmic Dawn ($z \sim 8.5$): $450,000\text{ M}_\odot$ Quasi-Star seed encased in a $60\text{ AU}$ pure hydrogen cocoon with active gravitational lensing. |

---

## 🛠️ Interactive Planet Builder GUI (`[P]`)

Open the floating Planet Builder sidebar anytime by pressing **`[P]`** or clicking **`[ 🪐 Planet Builder [P] ]`**:
- **9 Physical Archetypes**: Earth-like, Jupiter-like, Super-Jupiter, Mega-Jovian, Water World, Molten Protoplanet, Ice Giant, Rogue Invader, and Red Dwarf Companion Star.
- **Interactive Fine-Tuning**:
  - Mass steppers (`÷10`, `÷2`, `×2`, `×10`).
  - Semi-major axis controls (`-1.0 AU`, `-0.2 AU`, `+0.2 AU`, `+1.0 AU`).
  - Eccentricity cycling (Circular $e=0.0 \to$ Moderate $e=0.15 \to$ High $e=0.60 \to$ Hyperbolic $e=1.25$).
  - Composition mix cycling (Rocky Silicate $\to$ Volatile Water/Ice $\to$ Metallic Iron $\to$ Gas Envelope).
- **Dual Insertion Modes**:
  - 🚀 **`[ 🚀 Insert into Orbit ]`**: Computes exact Keplerian circular velocity $v_{\text{circ}} = \sqrt{\frac{G M_*}{a}}$ for instant orbital insertion.
  - 🎯 **`[ 🎯 Click-in-3D Mode ]`**: Raycasts directly against the orbital plane ($Y = 0$), placing your custom world wherever you click in 3D space.

---

## 🕹️ Controls & Hotkeys

### Navigation & Camera
| Action | Key / Mouse | Description |
| :--- | :---: | :--- |
| **Pan Camera** | `W`, `A`, `S`, `D` | Move the camera focus across the orbital plane |
| **Orbit Camera** | `Right Click + Drag` | Smooth 3D spherical orbit around the focus point |
| **Zoom In / Out** | `Mouse Scroll` | Exponentially smoothed logarithmic zoom ($0.005\text{ AU}$ to $250,000\text{ AU}$) |
| **Select / Focus** | `Left Click` | Click any celestial body to lock camera focus with zero lag |
| **Cycle Focus** | `Tab` / `Shift+Tab` | Distance-ordered cycling across all system worlds |
| **Reset View** | `R` / `Escape` | Reset camera focus to the central star |
| **Toggle Quick Bar** | `H` | Collapse / expand the top celestial body switcher bar |
| **Adjust Size Scale** | `.` / `,` | Exaggerate or normalize celestial body visual radii |

### Simulation & Time Warp
| Action | Key | Description |
| :--- | :---: | :--- |
| **Pause / Resume** | `Spacebar` | Pause or resume physical integration |
| **Time Warp Down / Up** | `←` / `→` | Step through simulation speeds: $1\times, 10\times, 100\times, 1,000\times, 10,000\times$ |
| **Warp Presets** | `1`, `2`, `3`, `4` | Direct jump to $1\times, 10\times, 100\times$, or $1,000\times$ simulation speed |

### Scenarios & Astrophysics Experiments
| Action | Key | Description |
| :--- | :---: | :--- |
| **Load Solar Nebula** | `F1` | 4.5 Gyr MMSN protostellar disk (Milky Way skybox) |
| **Load TRAPPIST-1** | `F2` | 7 Resonant Earths with 3 Habitable Zone worlds |
| **Load Kepler-16** | `F3` | Circumbinary binary star pair with giant & exomoon |
| **Load Hot Jupiter** | `F4` | Inward gas giant disk migration |
| **Load Rogue Planet** | `F5` | Hyperbolic interstellar invader flyby |
| **Load Little Red Dot** | `F6` | $450,000\text{ M}_\odot$ Quasi-Star with Early Universe Cosmic Web & Gravitational Lensing |
| **Planet Builder GUI** | `P` | Open / close floating Planet Builder sidebar |
| **Trigger LHB** | `G` | Trigger Late Heavy Bombardment & cometary water delivery |
| **Super-Eddington Toggle** | `X` | Toggle $4.5\times$ vs $0.9\times$ black hole seed accretion rate |
| **Cocoon Blowout** | `B` | Blow away hydrogen cocoon $\to$ contract lens down to $2.5\text{ AU}$ naked black hole |

---

## 🏗️ Architecture

Protostellar is designed around Bevy's data-driven **Entity Component System (ECS)**:

```
protostellar/
├── assets/
│   └── shaders/
│       ├── skybox.wgsl           # Procedural Milky Way & Early Universe Cosmic Web + Gravitational Lensing
│       ├── planet.wgsl           # PBR crusts, magma oceans, Rayleigh/Mie atmospheres, Jovian storms
│       ├── gas_cloud.wgsl        # 15-layer flared 3D protoplanetary gas disk
│       ├── planetary_rings.wgsl  # Optical depth and micro-ringlets shader
│       └── particle_render.wgsl  # GPU instanced particle swarm rendering
├── src/
│   ├── simulation/               # Symplectic leapfrog physics, accretion, thermodynamics, scenarios
│   ├── rendering/                # Procedural shaders, celestial meshes, camera, effects, skybox
│   ├── game/                     # UI overlays, Planet Builder HUD, time control, interaction
│   └── utils/                    # Astronomical constants, math solvers, orbital mechanics
└── tests/
    └── simulation_tests.rs       # 46 rigorous automated astrophysics and climate tests
```

---

## ⚡ Getting Started

### Prerequisites
- **Rust Toolchain**: Stable Rust 1.80+ installed via [rustup.rs](https://rustup.rs/).
- **GPU**: Hardware support for Metal (macOS Apple Silicon / Intel), Vulkan (Linux / Windows), or DirectX 12.

### Building & Running
Clone the repository and run in optimized release mode:

```bash
git clone git@github.com:7empest462/protostellar.git
cd protostellar
cargo run --release
```

> **Note:** Always compile with `--release`! Protostellar's numerical integrators, GPU pipeline, and 50,000-particle swarms are heavily optimized for release builds, running locked at 120+ FPS on Apple Silicon M-series chips and modern GPUs.

### Running Automated Astrophysics Tests
To run all 46 unit and integration tests:

```bash
cargo test --test simulation_tests
```

---

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
