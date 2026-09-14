# PROTOSTELLAR 🪐✨

**Protostellar** is a high-performance, visually stunning Solar System and Exoplanet Formation Simulator built in Rust using the [Bevy Engine](https://bevyengine.org/). It combines rigorous astrophysical modeling with real-time procedural rendering, allowing you to watch 100,000+ particles organically coalesce into stars, terrestrial worlds, ringed gas giants, relativistic remnants, and intricate moon systems over millions of simulated years.

---

## 🌟 Key Features & Astrophysical Engine

### 1. 🌌 N-Body Gravitational Dynamics & Symplectic Leapfrog Integration
- **High-Performance Physics**: Simulates 100,000+ GPU compute particles and active celestial bodies simultaneously using multi-threaded spatial partitioning.
- **Symplectic Leapfrog Integrator**: Guarantees long-term orbital energy and angular momentum conservation across millions of years, even at extreme time-warps up to $10,000\text{x}$.
- **Heliocentric Indirect Gravity (Jacobi Correction)**: Implements the exact celestial mechanics indirect term $-\frac{G M_j}{R_j^3} \vec{R}_j$ for star-centered coordinate frames, ensuring multi-star systems and cluster companions (e.g. Kepler-16B, LBV 1806-20) exert physical tidal forces without unphysical coordinate drift.
- **Bondi-Hoyle & Runaway Jovian Accretion**: Models gravitational capture radii, Hill sphere sweeping, and two-stage core accretion transitioning rocky cores into massive gas giants and stars without unphysical premature mass runaway.

### 2. 🌕 Theia-Earth Giant Impact & Moon Formation (`[M]`)
- **Oblique Giant Impact Regime**: Accurately simulates the canonical collision between Proto-Earth and Mars-sized embryo Theia at $T \sim 50\text{--}100\text{ yr}$ (or on-demand via `[M]`).
- **Stable Prograde Lunar Orbit**: Silicate mantle debris coalesces into **The Moon** in a stable prograde circular orbit ($a = 0.0075\text{ AU}$, safely outside Earth's visual atmospheric halo).
- **Planetary Spin-Up & Obliquity**: The impact imparts angular momentum, spinning Earth up to a rapid $6.0\text{-hour}$ day length with an authentic $23.4^\circ$ axial tilt.
- **Deep Mantle LLSVP Enrichment**: Models the burial of dense Theia iron-silicate mantle fragments into the core-mantle boundary, forming Earth's primordial Large Low Shear Velocity Provinces (LLSVPs) with $+2.8\%$ density contrast.
- **Deep-Space Recovery & Satellite Immunity**: Incorporates orbital trajectory interception that automatically catches and recovers stranded embryos, paired with unconditional parent-satellite collision exclusion to prevent newly formed moons from ever being destroyed or bounced into deep space.

### 3. 📊 Interactive Scientific Telemetry Panel (`[T]`)
- **System Conservation Auditing**: Real-time monitoring of total kinetic, potential, and mechanical energy with relative error tracking ($\Delta E / E_0$) to verify symplectic conservation.
- **Real-Time Sparklines**: Live ASCII/graphical sparklines tracking energy fluctuations, system angular momentum, and active celestial body counts.
- **Census & Mass Distribution**: Instantaneous breakdown of stars, terrestrial planets, gas giants, protoplanetary embryos, and minor bodies.
- **CSV Data Export**: One-click telemetry export directly to `telemetry_export.csv` for post-simulation scientific analysis.

### 4. 🔬 Interactive Celestial Inspector Panel (`[I]`)
- **Deep Geophysical Profiling**: Select any body in the simulation to inspect its internal and atmospheric parameters in real time:
  - Total mass ($M_\odot$ and $M_\oplus$), physical radius, mean density, and surface gravity ($g$).
  - Surface temperature, blackbody equilibrium temperature, and radiative balance.
  - Compositional inventory (silicate rocks, iron core, water/ice volatile fraction, and hydrogen/helium envelope).
  - Internal differentiation (core-mantle-crust mass fractions and geodynamo magnetic shielding).
  - Rotation period (day length) and orbital mechanics elements ($a$, $e$, $i$, $\Omega$, $\omega$, $\nu$).

### 5. 🕳️ General Relativistic Gravitational Lensing & Spacetime Warping
- **Ray-Deflection Optics**: Accurately computes Einstein deflection angles $\alpha(\theta) = \frac{\theta_E^2}{\theta}$ for massive black holes, bending background stars, cosmic web filaments, and distant nebulae into genuine **Einstein rings and gravitational arcs**.
- **Photon Sphere Caustic Rings**: Renders the razor-sharp caustic ring at $r \approx 1.5 R_s$ where orbiting photons escape, complete with relativistic Doppler beaming asymmetry.
- **Kerr Spacetime Geodesic Funnels**: Visualizes 12 spiraling geodesic field lines tracing frame-dragging and the spatial metric curvature funneling into the singularity.
- **Deep-Zoom Event Horizon Exception**: Dedicated camera clearance allowing deep-zoom down to $0.001\text{ AU}$ exclusively on the JWST Little Red Dot black hole star to inspect general relativity up close, while retaining safe surface collision protection on stars and planets.
- **Dynamic Cocoon Blowout Contraction**: When radiation pressure blows away the $60\text{ AU}$ Little Red Dot cocoon, the entire gravitational lens smoothly contracts down to $2.5\text{ AU}$ around the naked event horizon.

### 6. 🧲 Extreme Stellar Remnants: Pulsars & Magnetars
- **PSR B1257+12 Millisecond Pulsar (`[F7]`)**:
  - Relativistic $161\text{ Hz}$ neutron star spin ($6.22\text{ ms}$ rotation period) with $10^9\text{ Gauss}$ magnetic field.
  - Dual sweeping conical relativistic synchrotron emission beams and equatorial light cylinder boundary.
  - Historical zombie exoplanet triad: Draugr ($0.02\text{ M}_\oplus$), Poltergeist ($4.3\text{ M}_\oplus$), and Phobetor ($3.9\text{ M}_\oplus$).
- **SGR 1806-20 Ultra-Magnetized Magnetar (`[F9]`)**:
  - $10^{15}\text{ Gauss}$ surface magnetic field (strongest in the known Universe) with periodic starquake reconnection flares.
  - Procedural 3D poloidal magnetic dipole flux ribbons spanning $5.5\text{ AU}$ and glowing incandescent equatorial plasma ring.
  - Extreme cluster system: Valkyrie (0.48 AU shattered iron core), Pyre (0.85 AU chthonian magma world), SGR Ejecta Clump $\alpha$ (1.65 AU), and luminous blue variable hypergiant companion LBV 1806-20 (18.0 AU).

### 7. ⚡ 100k GPU Compute Particle Swarm Architecture
- **WebGPU Compute Pipeline**: Offloads particle orbital dynamics, flared gas drag, 32-body N-body perturbations, and shockwaves to `particle_orbit.wgsl` with workgroup size 64.
- **Asynchronous Double-Buffered Staging**: Zero CPU stalls via a 3-state async staging state machine (`IDLE` $\to$ `MAPPING` $\to$ `MAPPED`), sustaining 100,000+ particles at locked 120 FPS on Apple Silicon Metal, Vulkan, and DirectX 12.
- **Dynamic Toggle (`[F8]`)**: Seamless hot-swapping between GPU compute pipeline and multi-threaded CPU symplectic fallback.

### 8. ☄️ Pebble Accretion, Streaming Instability & Asteroid Belts
- **Sub-Millimeter Aerodynamic Pebble Drift**: Gas-drag-induced inward drift of pebbles across the protoplanetary disk, trapping volatile ices at the snow line.
- **Streaming Instability Planetesimal Seeding**: High local dust-to-gas ratios trigger spontaneous gravitational collapse into 1,024-body Main Asteroid and Kuiper Belt swarms.
- **Impact Cratering & Viscous Relaxation**: High-velocity impacts excavate transient impact basins, delivering volatiles and dynamically relaxing through viscoelastic mantle flow over million-year geological epochs.

### 9. 🪐 Dynamic Roche Disruption & Planetary Ring Spawning
- **Fluid Roche Limit Shredding**: When a moon, comet, or planetesimal ventures inside a primary's fluid Roche limit $d_{\text{Roche}} \approx 2.44 R_p \left(\frac{\rho_p}{\rho_s}\right)^{1/3}$, tidal forces overcome self-gravity, stretching the body into an ellipsoid before shattering it.
- **Keplerian Spiral Debris Streams**: Renders expanding, 48-fragment Keplerian debris streamers wrapping around the planet's equatorial plane, simulating the dynamic transition from fragmented rubble into a circularized ring plane over time.
- **Composition-Dependent Ring Albedo**: Rings dynamically adopt the color and optical depth of the disrupted body:
  - Brilliant reflective silver-white (`ice >= 70%`, Saturn-like).
  - Warm sand-cream tones (`35% - 70%` ice/dust mixture).
  - Dark charcoal / anthracite (`< 35%` ice, Uranus/Jupiter-like silicate rings).

### 10. 💨 Atmospheric Photoevaporation & Cometary Outflow Tails
- **Extreme UV (EUV) Hydrodynamic Escape**: Close-in worlds ($a < 0.25\text{ AU}$) absorb high-energy stellar flux, driving supersonic Parker-type hydrodynamic winds.
- **Energy-Limited Mass Stripping**: Strips hydrogen/helium envelopes down to bare chthonian rocky cores over geological time, reproducing the observed exoplanet "Hot Neptune Desert".
- **3D Anti-Stellar Cometary Tails**: Supersonic ion core spines and parabolic bow shocks trailing with orbital aberration, glowing in electric cyan or incandescent mineral vapor.

### 11. 🌌 Procedural Celestial Skybox
- **Milky Way & Star Clusters**: Tilted galactic frame ($60.2^\circ$), glowing Sagittarius A* galactic core, Great Rift molecular dust absorption lanes, and open clusters (Pleiades).
- **Early Universe High-Redshift Cosmic Web** *(Little Red Dot, $z \sim 8.5$)*: Primeval intergalactic filaments glowing with redshifted Lyman-$\alpha$ emissions, Strömgren ionization bubbles, and Population III hypergiant star clusters.

---

## 🚀 Sandbox Scenarios & Presets

Switch between multi-system presets instantly via the top HUD bar or function keys (`F1`–`F9`):

| Preset | Key | Description |
| :--- | :---: | :--- |
| **Hayashi Solar Nebula** | `[F1]` | Canonical 4.5 Gyr Minimum Mass Solar Nebula (MMSN) with central protostar and seeded protoplanetary niches. |
| **TRAPPIST-1 System** | `[F2]` | Ultracool M-dwarf with 7 resonant Earth-sized worlds in a compact Laplace chain (3 in the liquid water habitable zone). |
| **Kepler-16 Circumbinary** | `[F3]` | "Tatooine" K/M-dwarf binary pair orbited by a Saturn-mass circumbinary giant with a habitable exomoon and outer ocean world. |
| **Hot Jupiter Migration** | `[F4]` | Massive $1.4\text{ M}_{\text{Jup}}$ gas giant undergoing Type II disk torque inward migration from $5.2\text{ AU}$ down to $0.045\text{ AU}$. |
| **Rogue Planet Flyby** | `[F5]` | Unbound $3.5\text{ M}_{\text{Jup}}$ interstellar interloper screaming through the system at $38\text{ km/s}$, scattering comets and tilting orbits. |
| **JWST Little Red Dot** | `[F6]` | Cosmic Dawn ($z \sim 8.5$): $450,000\text{ M}_\odot$ Quasi-Star seed encased in a $60\text{ AU}$ pure hydrogen cocoon with active gravitational lensing. |
| **PSR B1257+12 Pulsar** | `[F7]` | Relativistic $161\text{ Hz}$ millisecond pulsar with synchrotron lighthouse beams & 3 zombie planets (Draugr, Poltergeist, Phobetor). |
| **SGR 1806-20 Magnetar** | `[F9]` | Ultra-magnetized $10^{15}\text{ G}$ magnetar with starquake flares, 3D magnetic flux loops, and LBV 1806-20 hypergiant cluster companion. |

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
| **Pan Camera** | `W`, `A`, `S`, `D` | Move camera focus across orbital plane |
| **Orbit Camera** | `Right Click + Drag` | Smooth 3D spherical orbit around focus point |
| **Zoom In / Out** | `Mouse Scroll` | Exponentially smoothed logarithmic zoom ($0.001\text{ AU}$ to $250,000\text{ AU}$) |
| **Select / Focus** | `Left Click` | Click any celestial body or top quick bar button to lock camera focus |
| **Cycle Focus** | `Tab` / `Shift+Tab` | Distance-ordered cycling across all system worlds |
| **Reset View** | `R` / `Escape` | Reset camera focus to the central star |
| **Toggle Quick Bar** | `H` | Collapse / expand the top celestial body switcher bar |
| **Toggle Orbit Trails** | `Y` | Cycle orbit visualization (`All Worlds` $\to$ `Selected Only` $\to$ `Hidden`) |
| **Adjust Size Scale** | `.` / `,` | Exaggerate or normalize celestial body visual radii |
| **Toggle Fullscreen** | `F11` | Toggle borderless fullscreen display |

### Simulation & Analysis
| Action | Key | Description |
| :--- | :---: | :--- |
| **Pause / Resume** | `Spacebar` | Pause or resume physical integration |
| **Time Warp Down / Up** | `←` / `→` | Step through simulation speeds: $1\times, 10\times, 100\times, 1,000\times, 10,000\times$ |
| **Warp Presets** | `1`, `2`, `3`, `4` | Direct jump to $1\times, 10\times, 100\times$, or $1,000\times$ simulation speed |
| **Telemetry Panel** | `T` | Open / close real-time energy conservation and orbital telemetry |
| **Celestial Inspector** | `I` | Open / close geophysical composition and structure inspector |
| **Trigger Theia Giant Impact** | `M` | Initiate Theia intercept to form The Moon and spin up Earth |
| **Trigger LHB** | `G` | Trigger Late Heavy Bombardment & cometary water delivery |
| **Planet Builder GUI** | `P` | Open / close floating Planet Builder sidebar |

### Scenarios & Astrophysics Experiments
| Action | Key | Description |
| :--- | :---: | :--- |
| **Load Solar Nebula** | `F1` | 4.5 Gyr MMSN protostellar disk (Milky Way skybox) |
| **Load TRAPPIST-1** | `F2` | 7 Resonant Earths with 3 Habitable Zone worlds |
| **Load Kepler-16** | `F3` | Circumbinary binary star pair with giant & exomoon |
| **Load Hot Jupiter** | `F4` | Inward gas giant disk migration |
| **Load Rogue Planet** | `F5` | Hyperbolic interstellar invader flyby |
| **Load Little Red Dot** | `F6` | $450,000\text{ M}_\odot$ Quasi-Star with Cosmic Web & Gravitational Lensing |
| **Load Pulsar System** | `F7` | Relativistic $161\text{ Hz}$ millisecond pulsar with lighthouse beams |
| **Toggle GPU Compute** | `F8` | Hot-swap between 100k GPU compute particles and CPU fallback |
| **Load Magnetar** | `F9` | $10^{15}\text{ G}$ magnetar with reconnection flares & 3D flux loops |
| **Super-Eddington Toggle** | `X` | Toggle $4.5\times$ vs $0.9\times$ black hole seed accretion rate |
| **Cocoon Blowout** | `B` | Blow away hydrogen cocoon $\to$ contract lens down to naked black hole |

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
│       ├── particle_orbit.wgsl   # GPU compute shader for 100k particle orbital dynamics & gas drag
│       └── particle_render.wgsl  # GPU instanced particle swarm rendering
├── src/
│   ├── gpu/                      # WebGPU compute nodes, double-buffered async staging buffers
│   ├── simulation/
│   │   ├── accretion/            # Collision regimes, Theia giant impact, Roche disruption, moon capture
│   │   ├── components/           # ECS component definitions (Mass, Velocity, Composition, SpinState, etc.)
│   │   ├── disk/                 # Protoplanetary disk structure, gas drag, streaming instability, belts
│   │   ├── scenarios/            # Preset system configurations (Solar Nebula, TRAPPIST-1, Magnetar, etc.)
│   │   ├── pebble_accretion.rs   # Aerodynamic pebble drift and core growth
│   │   ├── telemetry.rs          # Energy conservation, angular momentum auditing, and CSV logging
│   │   ├── physics.rs            # Symplectic leapfrog integrator, N-body gravity
│   │   ├── thermodynamics.rs     # Stellar evolution, climate, habitability modeling
│   │   └── resources.rs          # Shared simulation state and configuration
│   ├── rendering/
│   │   ├── bodies/               # Celestial body meshes, palettes, ring structures
│   │   ├── effects/              # Orbital ribbons, conics, cometary tails, lensing
│   │   ├── particle_swarm/       # CPU billboard rendering, particle simulation fallback
│   │   ├── camera.rs             # Pan-orbit camera, zoom, body tracking
│   │   ├── gas_clouds.rs         # Volumetric protoplanetary disk rendering
│   │   └── skybox.rs             # Procedural skybox (Milky Way / Cosmic Web)
│   ├── game/
│   │   ├── ui/                   # HUD panels, inspector, Planet Builder, telemetry panel
│   │   ├── interaction.rs        # Selection, click-to-place, keyboard input
│   │   ├── phases.rs             # Formation phase state machine
│   │   └── time_control.rs       # Time warp, pause, step-once
│   └── utils/                    # Astronomical constants, math solvers, orbital mechanics
└── tests/
    ├── simulation_tests.rs       # Test harness entry point
    └── simulation_tests/         # 151 rigorous automated astrophysics, climate, and stability tests
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

> **Note:** Always compile with `--release`! Protostellar's numerical integrators, GPU compute pipeline, and 100,000-particle swarms are heavily optimized for release builds, running locked at 120+ FPS on Apple Silicon M-series chips and modern GPUs.

### Running Automated Astrophysics Tests
To run all 151 unit and integration tests:

```bash
cargo test --test simulation_tests
```

---

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
