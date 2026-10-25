# PROTOSTELLAR 🪐✨

**Protostellar** is a high-performance, visually stunning Solar System and Exoplanet Formation Simulator built in Rust using the [Bevy Engine](https://bevyengine.org/). It combines rigorous astrophysical equations of motion with real-time procedural GPU shaders, allowing you to witness 100,000+ particles organically coalesce into stars, rocky worlds, ringed gas giants, relativistic remnants, and intricate moon systems over millions of simulated years.

---

## 📖 Table of Contents
1. [🌟 Recent Additions & Major Engines](#-recent-additions--major-engines)
2. [🎮 Flight Manual & How to Access Everything In-Game](#-flight-manual--how-to-access-everything-in-game)
   - [Camera & Viewport Controls](#camera--viewport-controls)
   - [Time Flow Engine](#time-flow-engine)
   - [Surface Horizon Mode & Procedural 3D Terrains](#surface-horizon-mode--procedural-3d-terrains)
   - [Galilean Moon Volcanism, Cryotectonics & Multi-Moon Eclipses](#galilean-moon-volcanism-cryotectonics--multi-moon-eclipses)
   - [Cosmic Hazards & Targeted Cometary Delivery](#cosmic-hazards--targeted-cometary-delivery)
   - [Giant Molecular Clouds & Volumetric Fluid Dynamics](#giant-molecular-clouds--volumetric-fluid-dynamics)
   - [Interstellar Probes & Astronautics](#interstellar-probes--astronautics)
   - [Exoplanet Civilizations & Megastructures](#exoplanet-civilizations--megastructures)
   - [Supernova Core-Collapse, Matter Ejection & Remnant Formation](#supernova-core-collapse-matter-ejection--remnant-formation)
   - [Persistent Supernova Remnant (SNR) Nebulae & Shaders](#persistent-supernova-remnant-snr-nebulae--shaders)
   - [Pristine Disk Genesis & Protostellar Thermonuclear Ignition](#pristine-disk-genesis--protostellar-thermonuclear-ignition)
   - [Secular Kozai-Lidov Resonance & Hierarchical Triples](#secular-kozai-lidov-resonance--hierarchical-triples)
   - [Contextual Scenario-Specific HUD Adaptation](#contextual-scenario-specific-hud-adaptation)
   - [Interactive Orbital Slingshot Launcher](#interactive-orbital-slingshot-launcher)
   - [Real-Time Trajectory Predictor & Encounter Forecaster](#real-time-trajectory-predictor--encounter-forecaster)
   - [Deep-Time Geological Epoch Scrubber & Continental Drift](#deep-time-geological-epoch-scrubber--continental-drift)
   - [Targeted Terraforming & Guided Planetary Bombardment](#targeted-terraforming--guided-planetary-bombardment)
   - [Relativistic Polar Jets & Synchrotron Light Cones](#relativistic-polar-jets--synchrotron-light-cones)
   - [Multi-Moon Solar Eclipses & Planetary Ring Shadows](#multi-moon-solar-eclipses--planetary-ring-shadows)
   - [Viscoelastic Tidal Heating & Spin-Orbit Locking](#viscoelastic-tidal-heating--spin-orbit-locking)
   - [General Relativistic Dynamics & Gravitational Wave Coalescence](#general-relativistic-dynamics--gravitational-wave-coalescence)
   - [Kilonova Fireballs, Binary Mergers & GW Ringdown](#kilonova-fireballs-binary-mergers--gw-ringdown)
   - [Realistic Black Hole Event Horizons & Optical Physics](#realistic-black-hole-event-horizons--optical-physics)
   - [Atmospheric Photoevaporation & Solar Wind Stripping](#atmospheric-photoevaporation--solar-wind-stripping)
   - [Dynamic Roche Disruption & Ring Spawner](#dynamic-roche-disruption--ring-spawner)
   - [Theia-Earth Giant Impact & Lunar Coalescence](#theia-earth-giant-impact--lunar-coalescence)
   - [Late Heavy Bombardment & Cometary Ocean Seeding](#late-heavy-bombardment--cometary-ocean-seeding)
   - [Interactive Planet Builder & Spawner GUI](#interactive-planet-builder--spawner-gui)
   - [Scrollable Target Inspector & Live Body Editor](#scrollable-target-inspector--live-body-editor)
   - [Full-System Telemetry & Habitability Graphing](#full-system-telemetry--habitability-graphing)
   - [System Save / Load & Scenario Serializer](#system-save--load--scenario-serializer)
   - [Universal HUD Minimization & Restore Pills](#universal-hud-minimization--restore-pills)
3. [🌌 Sandbox Scenario Library](#-sandbox-scenario-library)
4. [⌨️ Master Controls & Shortcuts Reference Card](#️-master-controls--shortcuts-reference-card)
5. [🏗️ Project Architecture](#️-project-architecture)
6. [🧪 Automated Test Suite (440 Tests)](#-automated-test-suite-440-tests)
7. [⚡ Getting Started](#-getting-started)
8. [📜 License](#-license)

---

## 🌟 Recent Additions & Major Engines

Protostellar has evolved far beyond an N-body gravity toy into a comprehensive astrophysical sandbox. Here is a summary of all recent major systems:

### 1. Galactic Evolution & Active Galactic Nuclei (AGN)
- **True-Scale Supermassive Black Holes**: Supermassive black holes render at their authentic physical Schwarzschild radius ($R_s = 2GM/c^2 \approx 0.085\text{ AU}$ for Sagittarius A*'s $4.3 \times 10^6\ M_\odot$) with volumetric, Doppler-beamed relativistic MHD accretion disks and razor-sharp $1.5 R_s$ photon rings.
- **Tidal Disruption Events (TDEs)**: Stars plunging within the Roche limit of a black hole are spaghettified into glowing plasma ribbons that dynamically wind into the accretion disk.
- **Relativistic Jets**: AGN and Magnetars fire highly collimated relativistic jets with synchronized Synchrotron radiation.
- **Sagittarius A\* System**: Simulates the extreme relativistic 1PN orbits and periastron velocity boosts of the galactic center S-star cluster (S2, S4714, S62, S0-102) around the Milky Way's central supermassive black hole.

### 2. Giant Molecular Clouds & N-Body Spiral Galaxies
- **WGPU Gas Hydrodynamics & GMC Far-Field Compute**: Leverages GPU compute shaders (`gmc_far_field.wgsl`) for Eulerian fluid dynamics, shock compression, and tile-shared $O(N)$ far-field gravitational acceleration across thousands of forming star systems.
- **GPU N-Body Spiral Galaxies & Dark Matter Halos**: Simulates 100,000+ star particles bound inside analytical Navarro-Frenk-White (NFW) dark matter halos (`nbody_galaxy.wgsl`) with 3D gravimetric velocity field visualization and Andromeda–Milky Way galactic collisions.
- **Lin-Shu Spiral Density Waves**: Stars and gas align into non-winding, grand-design spiral arms (4-armed Milky Way style).
- **Jeans Instability & HII Ionization Bubbles**: Cold gas clumps dynamically fragment and collapse into new protostellar seeds while igniting stars carve glowing HII cavities in 3D raymarched nebulae.

### 3. Procedural Exoplanet Flora, Bioluminescence & Megastructures (Phase 7)
- **Spectral Photosynthetic Pigments**: Continental vegetation dynamically adapts its pigment palette to the host star's spectral energy distribution (`SolarChlorophyll` emerald-jade under G/F stars, `RedDwarfCrimson` near-infrared anthocyanin under M-dwarfs like TRAPPIST-1, `OrangeDwarfCarotenoid` amber-gold under K-dwarfs/binaries like Kepler-16, and `HighUvIndigo` UV-shielded scytonemin under hot stars and compact remnants).
- **Bioluminescent Twilight & Nightside Ecospheres**: Tidally locked and vegetated exoplanets illuminate their twilight terminators and nightsides with glowing bioluminescent forest canopy veins and coastal plankton blooms.
- **Gravity-Modulated Alpine Treelines**: Planetary surface gravity ($g / g_\oplus$) dynamically compresses or expands continental treeline elevations.
- **Dyson Swarms & Night-Side City Lights**: Advanced civilizations cast emissive urban networks on the night hemisphere and construct instanced Dyson Swarms that orbit and intercept their host star's luminosity.

### 4. Interstellar Probes, Geodesic Surface Rovers & Astrobiology Telemetry (Phase 7)
- **Autonomous Ion-Drive Spacecraft**: Launch `SpaceProbe` entities (Orbiters and Rovers) that compute intercept trajectories and execute continuous low-thrust ion burns.
- **Geodesic Rover Surface Navigation & 3D Trails**: Upon touchdown, Rovers autonomously traverse the rotating 3D planetary crust along great-circle geodesics with terrain-avoidance steering, rendering persistent amber-gold breadcrumb trails and pulsing cyan Raman laser core-drill beams.
- **Crustal Core Drilling & Orbital Spectrometry**: Rovers and Orbiters collect surface cores and atmospheric transmission spectra, reporting planetographic `Lat / Lon` coordinates, kilometers driven, and biosignature/mineralogical assays in the Target Inspector HUD.

### 5. Procedural 3D Surface Horizon Mode & Terrains
- **Surface Horizon Skimming**: Toggle into seamless first-person planetary surface exploration (**`[Shift + V]`** or **`[Surface]`** in Target Inspector) with horizon-locking pitch/yaw camera controls.
- **Multi-Scale Continental Terrains**: Dynamic procedural vertex-displaced continental elevations featuring canyons, mountain ranges, impact craters, and eroded ocean basins.
- **Dynamic 3D Vegetation Instancing**: Procedurally places thousands of instanced 3D plant meshes (broadleaf canopies, alpine conifers, alien flora) tuned to surface moisture, temperature, and host-star photosynthetic pigment classification.
- **Geodesic Rover Driving & Laser Core Drills**: Drive landed rovers with authentic terrain-hugging physics (**`W/A/S/D`** throttle/steer, **`Shift`** turbo boost, **`Space`** brake) leaving continuous wheel tracks and firing Raman spectrometry lasers.

### 6. Galilean Moon Volcanism, Cryotectonics & Multi-Moon Eclipses
- **Io Hyperactive Sulfur Volcanism**: Dynamic volcanic calderas, glowing silicate lava fountains, and radioactive sulfur dioxide fallout halos driven by viscoelastic tidal dissipation ($> 0.5\text{ W/m}^2$).
- **Europa & Enceladus Cryotectonics**: Procedural double-ridge fractures, tiger stripe lineae, and forward-scattering water-ice geysers that feed into and replenish parent planetary rings.
- **Simultaneous 4-Moon Solar Eclipse Shadows**: Upgraded GPU shadow buffers compute up to 4 concurrent transiting moons (`[Vec4; 4]`), casting accurate overlapping umbral and penumbral twilight spot shadows across planetary day/night faces.

### 7. Cosmic Hazards & Targeted Cometary Delivery
- **Guided Cometary Delivery**: Proportional navigation guidance sweeps cometary swarms on collision courses towards target planets, delivering volatile water budgets to condense surface oceans and build atmospheres.
- **Late Heavy Bombardment Overhaul**: Execution-ordered cometary cascades guaranteed to bombard inner protoplanets and transform dry rocky crusts into water-bearing worlds.

### 8. Physically Based Specular Optics & Planetary Realism
- **Neutral Dielectric Specular Optics**: Blinn-Phong specular glints with neutral white reflection ($F_0$) eliminating subtractive chromatic inversions that previously created artificial purple/blue glints on warm terracotta and orange planets.
- **Super-Puff / Sub-Neptune / Gas Dwarf Realism**: Physical composition overrides orbital labels: gas-rich worlds (`gas_frac > 0.20`, density $< 0.5\text{ g/cm}^3$) correctly simulate with deep, opaque `GasGiantEnvelope` cloud decks and atmospheric hazes rather than bare rocky continental relief, even when gravitationally captured as satellite companions.

### 9. Symplectic Physics & Memory Optimizations
- Fully allocation-free hot path: Eliminates thousands of `Vec` allocations per frame during symplectic N-body timestepping.
- High-warp multi-threading with Rayon `par_iter` and $O(N)$ index lookups for enormous performance gains during fast-forwarded cosmological timescales.

## 🎮 Flight Manual & How to Access Everything In-Game

### Camera & Viewport Controls
- **3D Orbit**: Hold **`Right Mouse Button`** and drag to rotate your camera around the current focus target in 360°.
- **Pan Viewport**: Use **`W`**, **`A`**, **`S`**, **`D`** to pan the focal center across the orbital plane.
- **Logarithmic Zoom**: Scroll your **`Mouse Wheel`** to zoom smoothly from deep-space intergalactic scales ($250,000\text{ AU}$) right down to planetary surface skimming ($0.001\text{ AU}$).
- **Select Celestial Body**: **`Left Click`** any star, planet, moon, or asteroid directly in 3D, or click its button in the top Quick Bar.
- **Cycle Focus Targets**: Press **`Tab`** to cycle forward through all system worlds ordered by distance from the star; press **`Shift + Tab`** to cycle backward.
- **Reset View**: Press **`Escape`** to deselect your current target, or press **`R`** to center the camera on the primary star.
- **Exaggerate Sizes**: Press **`.`** (period) to increase visual size exaggeration ($1.4\times$ per tap up to $20\times$), or press **`,`** (comma) to normalize body scales ($0.7\times$ per tap down to $0.1\times$).

---

### Time Flow Engine
Protostellar features a multi-tiered symplectic time integrator allowing you to watch rapid orbital passes or accelerate across million-year geological epochs:
- **`Spacebar`**: Pause / Resume physics integration.
- **`1`**: $1.0\times$ Real-time orbital speed.
- **`2`**: $100\times$ Accelerated orbital flow.
- **`3`**: $10,000\times$ High-speed planetary accretion flow (~$10\text{ kyr/s}$).
- **`4`**: $1,000,000\times$ Deep astronomical time warp (~$1\text{ Myr/s}$).
- **`←` / `→`**: Incrementally step simulation speed down or up.

---

### Surface Horizon Mode & Procedural 3D Terrains
Descend from orbital vantage points directly down to the surface of any terrestrial or rocky world to explore canyons, vegetation, and mountain peaks in real time:
- **First-Person Horizon Skimming**: Select any planet or moon and press **`[Shift + V]`** (or click **`[Surface [Shift-v]]`** in the Target Inspector). The camera snaps tangent to the planetary surface, orienting to local gravity and horizon-level forward motion.
- **Procedural Multi-Scale Geometry**: True 3D terrain meshes procedurally generated from continental FBM heightmaps, ridge noise, and erosion masks. Features deep canyons, impact crater basins, craggy mountain ridges, and smooth ocean floors.
- **Instanced 3D Vegetation**: Depending on the world's biomass, moisture, surface temperature, and stellar spectral type, the terrain dynamically populates with thousands of instanced 3D trees and plants (conifers, broadleaf forests, alien NIR anthocyanin shrubs) that scale with local treeline elevation and gravity.
- **Geodesic Rover Driving**: If a surface rover is deployed, take manual or autonomous control:
  - **`W` / `S`**: Forward drive throttle / reverse braking.
  - **`A` / `D`**: Steer left and right across the rotating planet crust.
  - **`Shift`**: Turbo drive boost.
  - **`Spacebar`**: Handbrake.
- **Exit Horizon View**: Press **`[Escape]`** or **`[Shift + V]`** to return to orbital camera mode.

---

### Galilean Moon Volcanism, Cryotectonics & Multi-Moon Eclipses
Experience the extreme geophysical diversity of the Jovian system:
- **Io Sulfur Volcanism**: Viscoelastic tidal flexing produces dynamic volcanic calderas, incandescent basaltic lava fissures ($1,200\text{ K}$), and widespread sulfur dioxide fallout halos ($250\text{ km}$ wide) across Io's surface.
- **Europa & Enceladus Cryotectonics**: Massive tidal heating fractures icy crusts into prominent double-ridge lineae, chaotic pack ice, and pressurized forward-scattering water-ice geysers.
- **Cryovolcanic Ring Feeding**: Cryovolcanic plumes on Enceladus continuously vent volatile water ice into orbit, replenishing and expanding Saturn's diffuse outer E-ring in real time.
- **Simultaneous 4-Moon Solar Eclipse Shadows**: Up to 4 transiting moons concurrently cast true geometrical umbras ($4\%$ sunlight) and penumbral falloffs across the host planet's atmosphere and cloud decks.

---

### Cosmic Hazards & Targeted Cometary Delivery
Simulate catastrophic asteroid swarms, cometary water delivery, and rogue impactors:
- **Guided Cometary Delivery**: Click **`[☄️ Launch Comet]`** in the Hazards panel (**`[Shift + H]`**) to dispatch high-velocity comets guided via proportional navigation. Swept closest-approach impact detection guarantees direct impacts on target worlds, outgassing dense steam atmospheres and condensing surface oceans.
- **Late Heavy Bombardment (LHB)**: Press **`[G]`** (or click **`[LHB [G]]`**) to trigger a system-wide gravitational resonance event. Outer gas giants fling swarms of volatile-rich Kuiper belt planetesimals inward, creating giant melt basins on Earth, Mars, and the Moon.

---

### Giant Molecular Clouds & Volumetric Fluid Dynamics
Immerse yourself in deep-space star formation through real-time 3D fluid volumetric raymarching. The simulation uses an Eulerian grid processed on the GPU to compute pressure, density, and turbulent velocity of cold hydrogen gas clouds.
- **Jeans Collapse & Protostar Ignition**: When gas density and temperature exceed the critical Jeans mass, localized regions gravitationally collapse, autonomously spawning Pop-I, II, or Pop-III massive hypergiants and advanced planetary systems depending on the cloud's primordial metallicity.
- **HII Ionization Bubbles**: As newborn stars ignite, their intense radiation pressure and stellar winds carve out massive, glowing cavities in the surrounding nebula.
- **To use**: Open the Scenarios Bar and click **`[Molecular Cloud]`**.

### Interstellar Probes, Surface Rovers & Astrobiology
Explore and survey your exoplanets by launching autonomous orbital and surface spacecraft:
- **Ion-Drive Navigation**: Probes compute interception trajectories and use continuous low-thrust ion drives to transit between worlds.
- **Orbital Spectrometry vs. Geodesic Surface Roving**: Upon arrival, Orbiters circularize into polar/inclined survey orbits to conduct IR/UV atmospheric transmission spectroscopy, while Rovers touch down on the crust, orient tangent to the terrain, and autonomously rove across the rotating planetary surface along great-circle geodesics.
- **3D Surface Trails & Raman Core Drilling**: Landed Rovers leave a rotating 3D breadcrumb trail on the planet's surface, fire a pulsing cyan Raman spectrometer laser into the regolith, and report real-time planetographic `Lat / Lon` coordinates, traverse distance (`km`), core sample counts, and biosignature findings in the Target Inspector.
- **To use**: Select any planet, open the Inspector (**`[I]`**), and click **`[Launch Probe]`**.

### Exoplanet Civilizations, Spectral Flora & Megastructures
Watch primitive biospheres adapt to their host star's spectrum and transition into advanced spacefaring empires:
- **Habitability & Spectral Flora Adaptation**: Worlds in the habitable zone with liquid oceans evolve photosynthetic flora tuned to their star's spectrum—emerald chlorophyll under Solar G-stars, deep crimson-burgundy NIR anthocyanins on M-dwarf worlds (`TRAPPIST-1`), amber-gold carotenoids on K-dwarf/circumbinary worlds (`Kepler-16`), and bioluminescent twilight/nightside canopies.
- **Dyson Swarms**: Once a civilization reaches a Type-II Kardashev threshold (a high technosignature), it constructs a massive Dyson Swarm around its host star that intercepts stellar radiation and measurably cools outer planets.
- **To use**: Select a habitable planet, open the Inspector (**`[I]`**), and click **`[Seed Life]`** or **`[Adv. Civ]`**, or wait millions of years for natural evolution.

### Supernova Core-Collapse, Matter Ejection & Remnant Formation
Witness the violent death of massive stars and the birth of exotic compact remnants through real-time 3D explosive hydrodynamic visuals:
- **Mass-Differentiated Astrophysical Regimes**:
  - **Hypernova / Collapsar ($\ge 25\ M_\odot$ - Relativistic Black Hole)**: Blast speeds reach $75\text{ AU/s}$ out to $150\text{ AU}$. Features dual relativistic polar breakout cones ($250,000\text{ K}$ sapphire-violet), massive core $^{56}\text{Ni}$ radioactive clumps, and leaves behind a stellar-mass Black Hole.
  - **Type II Core-Collapse ($8 - 25\ M_\odot$ - Pulsar / Neutron Star)**: Iron core-collapse neutrino detonation expanding at $45\text{ AU/s}$ out to $110\text{ AU}$. Yields concentric chemical shells: inner golden radioactive nickel-iron, middle $[\text{O III}]/\text{Si}/\text{S}$ emerald mantle, and outer $H\alpha$ ruby hydrogen envelope, leaving a rotating Pulsar.
  - **Type Ia Thermonuclear Detonation ($> 1.44\ M_\odot$ White Dwarf)**: Complete thermonuclear runaway consumption of the carbon-oxygen core expanding at $35\text{ AU/s}$ out to $95\text{ AU}$, thoroughly disrupting the star with zero remnant core.
  - **Planetary Nebula Thermal Pulse ($0.5 - 8\ M_\odot$ - White Dwarf)**: Gentle thermal envelope shedding at $4.5\text{ AU/s}$ out to $45\text{ AU}$ leaving a cooling degenerate White Dwarf.
- **Prompt Detonation Fireball & Rayleigh-Taylor Ejecta**:
  - Prompt shock breakout fireball begins at the star's photosphere radius rather than a point singularity, decaying exponentially ($I(t) = e^{-3.5 t}$).
  - 100+ discrete high-velocity matter clumps experience procedural Rayleigh-Taylor turbulent harmonic vorticity ($\sin/\cos$ curling) and trail filamentary streamer fingers back toward the center of explosion.
  - Ejecta clumps decelerate under interstellar medium resistance and cool down radiatively over time.
- **Circumstellar Disk & Planetary Blast Wave Interaction**:
  - The forward blast imparts a supersonic radial kick ($+15\text{ AU/s}$) to protoplanetary disk particles, vaporizes volatile ices (`ice_frac = 0.0`), and superheats circumstellar dust to an incandescent $4,500\text{ K}$.
  - Inner asteroids and comets within $2.0\text{ AU}$ of the core collapse are completely vaporized and despawned.
  - Surviving planets absorb outward momentum impulses, experience extreme surface heating, and have active cometary atmospheric ablation tails dynamically attached pointing radially away from the remnant.
- **In-Game Triggers**:
  - In the Target Inspector (**`[I]`**), advance a massive star's evolutionary state until it detonates.
  - Focus the central star and press **`[N]`** to trigger instant core collapse.

---

### Persistent Supernova Remnant (SNR) Nebulae & Shaders
Following prompt stellar core collapse, explosions smoothly transition into persistent, long-lived diffuse nebulae modeling centuries of interstellar blast dynamics:
- **Sedov-Taylor Blast Expansion**:
  - Remnants decouple from the prompt ballistic phase and transition into the self-similar adiabatic Sedov-Taylor blast regime:
    $$R(t) = R_0 \cdot \left(1 + \frac{t}{\tau_{\text{sedov}}}\right)^{0.38}$$
  - The forward shock decelerates under interstellar medium (ISM) resistance while continuing to sweep outward across the planetary system out to $250+\text{ AU}$.
- **Multi-Element Filamentary Wisps**:
  - As the ejecta decelerates, radiative cooling fragments the shell into structured, turbulent filamentary wisps.
  - Spectrally differentiated emission: ruby $H\alpha$ ($656\text{ nm}$) hydrogen filaments, emerald $[\text{O III}]$ ($500.7\text{ nm}$) doubly-ionized oxygen sheets, and amber $[\text{S II}]$ shock boundaries.
  - Remnants slowly fade over tens of thousands of simulated years ($e^{-t / \tau_{\text{cool}}}$) into deep-space interstellar medium enrichments.
- **Pulsar Wind Nebula (PWN) Synchrotron Core**:
  - When a Type II core collapse leaves behind a newborn Pulsar or Magnetar, relativistic electron-positron winds continuously inject energy into the inner cavity.
  - Generates a vibrant, persistent Crab-like synchrotron core emitting non-thermal sapphire glow energized by the central engine's spin-down luminosity.
- **Volumetric WGSL Supernova Shader (`supernova.wgsl`)**:
  - Custom ray-marched shader pass calculating real-time optical depth, inner photosphere absorption, forward blast ionization rims, and high-energy limb brightening.

---

### Pristine Disk Genesis & Protostellar Thermonuclear Ignition
Start from absolute cosmic scratch and watch planets assemble organically:
1. Load the **Disk Genesis** scenario via **`[Shift + F1]`** or clicking **`[Disk Genesis]`** in the Scenarios bar.
2. Unlike preset planetary systems, Genesis begins with only a central contracting protostar and a pristine Class II T-Tauri accretion disk of 100,000 SPH viscous gas and dust particles.
3. **Astrophysical Growth Mechanics**:
   - **Aerodynamic Gas Drag & Settling**: Fine grains settle to the midplane and experience sub-Keplerian headwind drift.
   - **Water-Ice Snow Line Trap ($2.7\text{ AU}$)**: Rapid volatile ice condensation forms a high-density pressure bump, accelerating pebble coagulation into rocky cores and gas giant seeds.
4. **Thermonuclear Core Ignition**:
   - The central protostar gravitationally contracts, building immense core temperature and pressure over ~100,000 to 200,000 simulated years.
   - Once core temperature reaches **$10.0\text{ MK}$ ($100\%$ fusion threshold)**, the protostar automatically ignites into a hydrogen-burning **Main Sequence Star**!
   - Core ignition unleashes a blinding shock breakout and intense solar wind radiation pressure that sweeps residual inner gas away into the outer system.
   - **Manual Hotkey Controls**:
     - Press **`[I]`** anytime to immediately ignite the protostar.
     - Press **`[Shift + I]`** (or press **`[I]`** on an ignited star) to trigger a violent Coronal Mass Ejection (CME) shockwave.

---

### Secular Kozai-Lidov Resonance & Hierarchical Triples
Experience three-body gravitational chaos and secular orbital evolution:
1. Load **`[F11]`** or click **`[HD 80606 (Kozai-Lidov Triple)]`** in the Scenarios bar.
2. An inner binary/planet orbits a primary star with a distant, highly inclined third companion (mutual inclination $i_{\text{mut}} > 39.2^\circ$).
3. **Quadrupole Secular Perturbations**:
   - Conserves the vertical component of orbital angular momentum:
     $$\sqrt{1 - e^2} \cos i = \text{const}$$
   - Drives large-amplitude cyclic oscillations between orbital inclination and eccentricity ($e \leftrightarrow i$).
   - Over secular timescales, the planet's orbit stretches from near-circular to extreme eccentricity ($e \to 0.88 - 0.93$), driving the periastron down to grazing distances where tidal dissipation, catastrophic heating, or Roche tidal disruption occur!

---

### Contextual Scenario-Specific HUD Adaptation
To preserve clarity and focus across wildly different astrophysical regimes, Protostellar features an intelligent contextual UI filter:
- **Scenario-Specific Menus**: Specialized action buttons only appear when relevant to the active scenario:
  - *Solar MMSN*: Late Heavy Bombardment (**`[G]`**), Theia-Moon giant collision (**`[M]`**).
  - *Disk Genesis*: Real-time protostellar core heating telemetry, manual thermonuclear ignition (**`[I]`**), CME triggers.
  - *JWST Little Red Dot*: Supermassive black hole accretion rates, Eddington ratio readouts, and gravitational lensing controls.
  - *Relativistic Binaries & Compact Remnants*: Synchrotron jet telemetry, 1PN precession readouts, and Peters GW inspiral triggers.
- **Universal Sandbox Tools**: Universal tools—such as the Interactive Orbital Slingshot (**`[K]`**), Trajectory Predictor (**`[N]`**), Planet Builder (**`[P]`**), Target Inspector (**`[I]`**), Scientific Telemetry (**`[F10]`**), and Time Integrator (**`Space`**, **`1`**-**`4`**)—remain accessible across all 11 scenarios.

---

### Interactive Orbital Slingshot Launcher
The slingshot launcher lets you test orbital stability, construct custom capture maneuvers, or trigger catastrophic collisions:
1. Press **`[K]`** or click **`[🎯 Slingshot [K]]`** in the top-right controls panel to enter Slingshot Mode.
2. A glowing reticle will lock onto your cursor on the orbital plane ($Y=0$).
3. Press **`[C]`** to cycle the payload archetype:
   - 🪨 **Asteroid**: High silicate fraction, low mass ($10^{-8}\text{ M}_\oplus$).
   - ☄️ **Icy Comet**: $80\%$ water-ice volatile fraction with outgassing tail.
   - 🌍 **Terrestrial Planet**: Differentiated Earth-mass silicate/iron world.
   - 🌊 **Water World**: Deep volatile ocean world ($40\%$ volatile fraction).
   - 🪐 **Gas Giant**: Jupiter-mass envelope with gas drag dynamics.
   - 🔴 **Rogue Planet**: $3.5\text{ M}_{\text{Jup}}$ unbound hypervelocity interloper.
4. **Click & Drag** to draw an impulse vector. The direction and length of your drag vector determine launch trajectory and velocity.
5. Release the mouse button to fire the projectile into orbit!

---

### Real-Time Trajectory Predictor & Encounter Forecaster
When piloting or inspecting worlds, Protostellar provides forward Keplerian conic prediction:
1. Press **`[N]`** or click **`[🎯 Forecast: ON [N]]`** in the top-right controls card.
2. The system propagates the selected body's orbit up to 500 steps into the future, rendering:
   - **Solid Turquoise Trajectory**: Bound elliptical or circular orbit ($e < 1.0$).
   - **Amber / Crimson Hyperbolic Trajectory**: Unbound flyby trajectory ($e \ge 1.0$).
   - **Encounter Forecaster (Target Inspector Readout)**:
     - 🟡 **Hill Sphere / Flyby Notice**: Warns when the path enters a neighbor's gravitational sphere of influence ($r < R_{\text{Hill}}$).
     - 🟠 **Roche Limit Warning**: Warns when tidal forces will disrupt the body ($r < d_{\text{Roche}}$).
     - 🔴 **Direct Impact Warning**: Calculates exact impact coordinates and target body.

---

### Deep-Time Geological Epoch Scrubber & Continental Drift
Observe planets evolve across deep geological time:
1. Select any terrestrial world (e.g. Earth in the Solar scenario `[F1]`).
2. Press **`[F11]`** or click **`[⏳ Epochs [F11]]`** in the controls panel.
3. The Geological Epoch Scrubber drawer slides down from the top:
   - **Hadean (4.56 - 4.00 Gyr)**: Boiling global magma ocean, glowing basaltic fractures, heavy asteroid bombardment.
   - **Archean (4.00 - 2.50 Gyr)**: Primordial crust cools, anoxic green iron-rich oceans form, micro-continents emerge.
   - **Proterozoic (2.50 - 0.54 Gyr)**: **Great Oxidation Event (GOE)** oxidizes dissolved iron, turning oceans from anoxic green to sapphire blue; Rodinia supercontinent aggregates.
   - **Phanerozoic / Paleozoic (541 - 252 Myr)**: Pangaea supercontinent aggregates; primitive terrestrial vegetation emerges on continents.
   - **Mesozoic (252 - 66 Myr)**: Supercontinent rifts and disperses into modern continental geometry; lush tropical vegetation spreads globally.
   - **Cenozoic / Anthropocene (66 Myr - Present)**: Modern continental layout, polar ice caps, and global biosphere coverage.
4. Click any epoch button (**`[Hadean]`**, **`[Archean]`**, **`[Proterozoic]`**, **`[Paleozoic]`**, **`[Mesozoic]`**, **`[Cenozoic]`**, **`[Modern]`**) or step time using **`[◄ -500 Myr]`** / **`[► +500 Myr]`**.
5. Toggle **`[▶ Auto-Advance]`** to watch continuous continental drift and oceanic oxidation playback!

---

### Targeted Terraforming & Guided Planetary Bombardment
Actively transform dry or hostile worlds into thriving biospheres:
1. Select your target world (e.g. Mars, Venus, or a custom rocky world).
2. Open the Target Inspector (**`[I]`**).
3. Scroll down to the **TERRAFORMING & BOMBARDMENT** section:
   - **`[☄️ Icy Comet]`**: Launches a $100\text{ km}$ water-ice comet from the outer system. On impact, it delivers volatile water, condensing surface oceans and building atmospheric pressure.
   - **`[🪨 Chondrite]`**: Fires a carbonaceous chondrite impactor rich in CO₂ and volatiles, inducing a greenhouse breakout on cold/frozen snowball worlds.
   - **`[⚡ Salvo]`**: Spawns an orbital cascade of 4 impactors to rapidly seed volatile inventories.
   - **`[💥 Core Impactor]`**: Direct metallic iron impact into the planet's core, stimulating geodynamo activity and establishing a protective magnetosphere ($> 0.35\text{ Gauss}$).

---

### Relativistic Polar Jets & Synchrotron Light Cones
Experience cutting-edge high-energy relativistic astrophysics:
1. Load **`[F7]`** (PSR B1257+12 Pulsar), **`[F9]`** (SGR 1806-20 Magnetar), or **`[F6]`** (JWST Little Red Dot Quasi-Star).
2. Observe dual polar relativistic jets rendered via a procedural volumetric WGSL shader:
   - **Kinematic Doppler Boosting**: Orbit your camera around the jet axis. As your view aligns with the jet direction ($\cos\theta \to 1.0$), the forward beam flares in an intense blazar/pulsar flash ($12\times - 20\times$ amplification), while the counter-jet dims down to $\le 0.08\times$.
   - **Braided Helical Flux Ropes**: Procedural double-helix magnetic flux ropes wrapping around the jet column.
   - **Supersonic Shock Knots (Mach Disks)**: Outward-traveling periodic shock wave packets moving at $v \approx 0.94 - 0.98c$.
   - **Lense-Thirring Precession**: Spinning beams precess in space, tracing glowing lighthouse cones.
3. Open the Target Inspector (**`[I]`**) to view live Lorentz factor ($\Gamma$), beaming half-angle ($\theta_{\text{beam}}$), and synchrotron luminosity.

---

### Multi-Moon Solar Eclipses & Planetary Ring Shadows
Real-time shadow projections rendered on planetary day/night hemispheres:
- **Ring Shadows**: Planets with ring systems (such as Saturn or worlds with shattered rings) project authentic planetary ring shadows onto their atmosphere and cloud decks, complete with day/night hemispheric alignment and the Cassini Division gap.
- **Multi-Moon Solar Eclipses**: When moons orbit between their parent planet and the central star, realistic umbral ($4\%$ sunlight) and penumbral ($4\% \to 100\%$ smooth transition) eclipse shadows sweep across the planet's surface in real time.

---

### Viscoelastic Tidal Heating & Spin-Orbit Locking
1. Select any close-in planet or moon (e.g. TRAPPIST-1 inner worlds, Io-like satellites).
2. Open the Target Inspector (**`[I]`**).
3. View the **TIDAL DYNAMICS & RHEOLOGY** readout:
   - Dissipation heating flux ($\text{W/m}^2$).
   - Tidal circularization timescale ($\text{Myr}$).
   - Spin-orbit synchronization state.
4. Click **`[🌊 Tidal Lock]`** to instantaneously circularize the orbit and lock the body's rotation into a synchronous 1:1 day-year resonance. When tidal heating exceeds $0.5\text{ W/m}^2$, surface magma oceans and volcanic hot spots erupt automatically!

---

### General Relativistic Dynamics & Gravitational Wave Coalescence
1. Load a compact remnant scenario or select a close-in body like Mercury.
2. In the Target Inspector (**`[I]`**), view the **RELATIVISTIC DYNAMICS** telemetry:
   - 1PN perihelion advance ($\text{arcsec/century}$).
   - Peters gravitational wave power ($P_{\text{GW}}$ in Watts).
   - GW inspiral coalescence timescale ($\tau_{\text{inspiral}}$ in Myr).
3. Click **`[🌀 Inspiral]`** on compact binary systems to accelerate gravitational radiation decay, pulling the components into the Innermost Stable Circular Orbit (ISCO) until they coalesce in a brilliant gravitational wave burst!

---

### Kilonova Fireballs, Binary Mergers & GW Ringdown
When compact relativistic binaries (binary neutron stars, neutron star-black hole pairs, or binary black holes) merge via Peters gravitational-wave orbital decay:
- **Binary Neutron Star (BNS) & NS-BH Coalescence**:
  - **Relativistic Polar Short-GRB Jets**: Merging neutron stars launch ultra-relativistic collimated gamma-ray burst jets ($\Gamma \ge 25$) piercing outward along the orbital angular momentum axis.
  - **r-Process Lanthanide Fireball**: Tidal dynamic ejecta and accretion disk wind outpourings synthesize rapid neutron-capture ($r$-process) heavy elements (gold, platinum, neodymium). The resulting radioactive fireball expands rapidly ($v \approx 0.1\text{--}0.3c$), cooling from an incandescent $5,000\text{ K}$ down to a deep infrared $2,000\text{ K}$ over tens of simulated days.
  - **Expanding Shock Blast Waves**: Supersonic multi-stage blast rings propagate through circumstellar gas, dispersing newly synthesized heavy metals into the surrounding planetary system.
- **Binary Black Hole (BBH) Coalescence & Ringdown**:
  - During the final inspiral plunge, extreme spacetime curvature ripples outward in concentric gravitational wave metric chirp ripples ($h_+ / h_\times$).
  - Post-merger, the single deformed Kerr black hole settles via **Quasi-Normal Mode (QNM) Ringdown**, releasing pure gravitational energy and producing optical spacetime shimmer before calming into a quiescent Kerr event horizon.

---

### Realistic Black Hole Event Horizons & Optical Physics
Black holes in Protostellar obey relativistic optical physics and Schwarzschild geometry:
- **Physical Schwarzschild Radius Scaling**:
  - Rather than artificially oversized spheres, black hole visual radiuses strictly scale with Schwarzschild radius:
    $$r_{\text{Schwarzschild}} = \frac{2 G M}{c^2}$$
  - A stellar-mass black hole ($3\text{--}10\ M_\odot$) collapses down to a compact core ($\sim 0.005\text{--}0.015\text{ AU}$), realistically tiny compared to its multi-AU progenitor supergiant star.
- **Pitch-Black Photon Absorption**:
  - The interior of the event horizon emits **zero lumens** ($I = 0.0$), acting as a pure, light-devouring void with unlit PBR properties.
- **Razor-Sharp Photon Ring & Gravitational Lensing**:
  - Relativistic ray lensing creates a razor-sharp photon ring ($1.5 R_s$) rendered via high-order grazing limb equations ($\propto (1 - \hat{N}\cdot\hat{V})^{36}$), contrasting against the pitch-black horizon.
  - All black holes—regardless of mass—actively warp and lens background star fields, Einstein rings, and surrounding nebula textures in real time.

---

### Atmospheric Photoevaporation & Solar Wind Stripping
1. Select any close-in gas-rich world ($a < 0.25\text{ AU}$).
2. In the Target Inspector (**`[I]`**), observe the EUV hydrodynamic escape rate ($\text{kg/s}$) and cometary outflow tail trailing anti-stellar.
3. Click **`[💨 Strip Atm]`** to simulate millions of years of extreme stellar wind ablation, stripping the volatile envelope down to a bare chthonian rocky core.

---

### Dynamic Roche Disruption & Ring Spawner
1. Select any moon, planetesimal, or comet orbiting close to a planet.
2. In the Target Inspector (**`[I]`**), click **`[💍 Shatter Rings]`** (or trigger **`[Spawn Sub-Roche Moon]`** in Planet Builder).
3. When inside the fluid Roche limit, tidal stresses shatter the body into expanding Keplerian spiral debris streams that circularize into a permanent planetary ring system matching the progenitor's composition (icy silver-white, dusty cream, or dark silicate).

---

### Theia-Earth Giant Impact & Lunar Coalescence
- Press **`[M]`** anytime (or trigger via the Inspector) to initiate the canonical Theia-Earth collision:
  - An embryo named **Theia** is placed on an incoming intercept trajectory towards Proto-Earth.
  - Upon impact, mantle debris is ejected into orbit, coalescing cleanly into **The Moon** in a stable prograde orbit ($a = 0.0075\text{ AU}$).
  - Earth absorbs kinetic energy, spinning up to a rapid $6.0\text{-hour}$ day length with an authentic $23.4^\circ$ axial tilt.
  - Theia's dense iron-silicate core fragments sink into the mantle, forming Earth's deep **Large Low Shear Velocity Provinces (LLSVPs)** ($+2.8\%$ density contrast).

---

### Late Heavy Bombardment & Cometary Ocean Seeding
- Press **`[G]`** (or click **`[☄️ Trigger LHB]`** in the Inspector) to initiate the Late Heavy Bombardment:
  - Outer giant planets experience resonant migration.
  - Hundreds of volatile-rich Kuiper belt planetesimals and comets are flung into the inner solar system.
  - High-velocity impacts excavate transient melt basins, outgassing dense steam atmospheres that cool and condense into Earth's first stable oceans.

---

### Interactive Planet Builder & Spawner GUI
Press **`[P]`** or click **`[🪐 Builder [P]]`** to open the full visual world architect:
- **11 Archetype Presets**: Earth-like, Super-Earth, Jupiter-like, Super-Jupiter, Heavy Super-Jupiter, Brown Dwarf, Water World, Molten Protoplanet, Ice Giant, Rogue Invader, and Red Dwarf Star.
- **Precision Steppers**: Adjust Mass ($\div 10$, $\div 2$, $\times 2$, $\times 10$) and Distance ($-1\text{ AU}$, $-0.2\text{ AU}$, $+0.2\text{ AU}$, $+1\text{ AU}$).
- **Eccentricity Cycling**: Circular ($e=0.0$) $\to$ Moderate ($e=0.15$) $\to$ High ($e=0.60$) $\to$ Hyperbolic ($e=1.25$).
- **Composition Mix**: Cycle through Rock, Ice, Metal, and Gas fractions.
- **Insertion Modes**:
  - **`[🚀 Insert into Orbit]`**: Automatically solves Keplerian circular velocity $v = \sqrt{GM_*/a}$ for immediate orbit injection.
  - **`[🎯 Click-in-3D Mode]`**: Click anywhere on the 3D plane to place your custom world.

---

### Scrollable Target Inspector & Live Body Editor
Press **`[I]`** to toggle the deep geophysical profiler:
- **Vertical Scrolling**: Use your mouse wheel or scrollbar to navigate long telemetry readouts.
- **Live Key-Based Editing**:
  - **`U` / `+`**: Increase body mass by $+25\%$.
  - **`J` / `-`**: Decrease body mass by $-20\%$.
  - **`O`**: Expand orbit radius by $+10\%$ (auto-recalculating circular velocity).
  - **`L`**: Contract orbit radius by $-10\%$.
  - **`C`**: Cycle physical composition (Silicate $\to$ Ice $\to$ Iron $\to$ Gas).
  - **`I` / `B`**: Forward velocity boost ($+15\%$).
  - **`K`**: Retrograde braking impulse ($-15\%$).
  - **`Z`**: Zero inclination and re-circularize orbit.
  - **`Delete` / `Backspace`**: Despawn selected body.
  - **`X`**: Vaporize body into a transient expanding debris cloud.

---

### Full-System Telemetry & Habitability Graphing
Press **`[F10]`** or click **`[📈 Telemetry [F10]]`** to open the Scientific Telemetry drawer:
- **Energy Conservation Auditing**: Live tracking of kinetic, potential, and total mechanical energy with relative symplectic error ($\Delta E / E_0$).
- **Multi-Metric Graphs**: Click metric buttons to graph:
  - Mechanical Energy $\Delta E / E_0$
  - System Angular Momentum
  - Active Celestial Body Count
  - Average Surface Temperature
  - Planetary Habitability Index
  - Total System Atmospheric Mass
- **Export to CSV**: Click **`[📊 Export CSV]`** to output full numerical telemetry logs directly to `telemetry_export.csv` for analysis in Python, MATLAB, or Jupyter.

---

### System Save / Load & Scenario Serializer
Save and resume your universe at any moment:
- **Quick Save**: Press **`[F12]`** or click **`[💾 Save [F12]]`** in the controls panel. Saves the entire simulation state to `saves/quicksave.json`.
- **Quick Load**: Press **`[Shift + F12]`** or click **`[📂 Load [S-F12]]`**. Reconstructs all celestial entities, physical components, and orbital vectors.

---

### Universal HUD Minimization & Restore Pills
Keep your viewport clean and cinematic:
- Every HUD container (Top-Left Telemetry, Top Quick Bar, Scenarios Bar, Controls Card, Target Inspector, Time Dock, Shortcuts Card) features a sleek **`🗕`** minimize button.
- Minimized panels collapse into glowing, non-intrusive restore pills docked at the screen edges.
- Click any pill (e.g. **`[📊 Stats ▼]`**, **`[🎬 Scenarios ▼]`**, **`[⏱️ Time Controls ▲]`**) to expand the panel back into view.

---

## 🌌 Sandbox Scenario Library

Switch scenarios instantly via the top Scenarios Bar or hotkeys (`F1`–`F11`):

| Preset | Key | Description |
| :--- | :---: | :--- |
| **Hayashi Solar Nebula** | `[F1]` | Canonical 4.56 Gyr Minimum Mass Solar Nebula (MMSN) with central protostar, Earth, Mars, Jupiter, and 1,024 asteroid planetesimals. |
| **Disk Genesis (Organic Planets)** | `[Shift+F1]` | Pristine Class II T-Tauri disk without starter planets. Aerodynamic gas drag, $2.7\text{ AU}$ snow line, and core contraction leading to $10.0\text{ MK}$ auto-ignition. |
| **TRAPPIST-1 System** | `[F2]` | Ultracool M-dwarf with 7 resonant Earth-sized worlds in a compact Laplace chain (3 in the liquid water habitable zone). |
| **Kepler-16 Circumbinary** | `[F3]` | "Tatooine" K/M-dwarf binary pair orbited by a Saturn-mass circumbinary giant with an exomoon and outer ocean world. |
| **Hot Jupiter Migration** | `[F4]` | Massive $1.4\text{ M}_{\text{Jup}}$ gas giant undergoing Type II disk torque inward migration from $5.2\text{ AU}$ down to $0.045\text{ AU}$. |
| **Rogue Planet Flyby** | `[F5]` | Unbound $3.5\text{ M}_{\text{Jup}}$ interstellar interloper screaming through the system at $38\text{ km/s}$, scattering comets and tilting orbits. |
| **JWST Little Red Dot** | `[F6]` | Cosmic Dawn ($z \sim 8.5$): $450,000\text{ M}_\odot$ Quasi-Star seed encased in a $60\text{ AU}$ pure hydrogen cocoon with active gravitational lensing. |
| **PSR B1257+12 Pulsar** | `[F7]` | Relativistic $161\text{ Hz}$ millisecond pulsar with synchrotron lighthouse beams & 3 zombie planets (Draugr, Poltergeist, Phobetor). |
| **SGR 1806-20 Magnetar** | `[F9]` | Ultra-magnetized $10^{15}\text{ G}$ magnetar with starquake flares, 3D magnetic flux loops, and LBV 1806-20 hypergiant cluster companion. |
| **PSR B1913+16 Relativistic Binary** | `[HUD]` | Hulse-Taylor binary pulsar ($1.44\ M_\odot + 1.38\ M_\odot$) in an eccentric $0.013\text{ AU}$ orbit demonstrating $4.22^\circ/\text{yr}$ 1PN periastron advance and GW inspiral decay. |
| **HD 80606 Kozai-Lidov Triple** | `[F11]` | Hierarchical triple system demonstrating periodic eccentricity and inclination exchange via the Kozai-Lidov mechanism ($e \to 0.88+$). |
| **Sagittarius A\* (Galactic Center)** | `[HUD]` | $4.3 \times 10^6\ M_\odot$ supermassive black hole at true Schwarzschild scale ($0.085\text{ AU}$) orbited by relativistic S-stars (S2, S4714, S62, S0-102) and G2 gas cloud. |
| **Giant Molecular Cloud (GMC)** | `[HUD]` | Volumetric 3D raymarched star-forming nebula with GPU Eulerian hydrodynamics, Jeans collapse, HII ionization bubbles, and GPU far-field cluster gravity. |
| **Milky Way & Andromeda Collision** | `[HUD]` | GPU N-body spiral galaxies bound in NFW dark matter halos with 4-arm Lin-Shu spiral density waves and tidal tail mergers. |

---

## ⌨️ Master Controls & Shortcuts Reference Card

### Viewport & Navigation
| Key / Input | Action |
| :--- | :--- |
| **`Right Click + Drag`** | Orbit camera 360° around focus target |
| **`W` / `A` / `S` / `D`** | Pan camera focal point |
| **`Mouse Scroll`** | Logarithmic zoom ($0.001\text{ AU} \to 250,000\text{ AU}$) |
| **`Left Click`** | Select celestial body or UI button |
| **`Tab`** / **`Shift + Tab`** | Distance-ordered target cycling |
| **`Escape`** | Deselect current body |
| **`R`** | Center view on primary star |
| **`Y`** | Cycle orbit trail mode (`All Worlds` $\to$ `Selected Only` $\to$ `Hidden`) |
| **`V`** | Cycle diagnostic overlay modes (`Realistic` $\to$ `Scientific` $\to$ `Minimal`) |
| **`.`** / **`,`** | Increase / decrease visual size exaggeration |
| **`H`** | Toggle quick body switcher bar |

### Simulation & Time
| Key / Input | Action |
| :--- | :--- |
| **`Spacebar`** | Pause / Resume physics integration |
| **`1`** / **`2`** / **`3`** / **`4`** | Time warp presets ($1\times$, $100\times$, $10,000\times$, $1,000,000\times$) |
| **`←`** / **`→`** | Step simulation speed down / up |

### Tools & Panels
| Key / Input | Action |
| :--- | :--- |
| **`P`** | Toggle Planet Builder & Spawner GUI |
| **`I`** | Toggle Target Inspector Panel (or ignite protostar / trigger solar flare when star is focused or in Genesis) |
| **`Shift + I`** | Trigger Coronal Mass Ejection (CME) solar blast wave |
| **`K`** | Toggle Interactive Orbital Slingshot Launcher |
| **`C`** | Cycle slingshot projectile archetype (in Slingshot mode) |
| **`N`** | Toggle Trajectory Predictor & Encounter Forecaster (or trigger core-collapse supernova when star is focused) |
| **`Shift + V`** | Toggle 3D Surface Horizon & Rover exploration mode |
| **`Shift + H`** | Toggle Cosmic Hazards Control Drawer |
| **`Shift + M`** | Toggle Campaign Objectives Panel |
| **`Shift + R`** | Toggle Hertzsprung-Russell (H-R) Stellar Diagram Panel |
| **`Shift + T`** | Toggle Technology Tree Panel |
| **`Shift + L`** | Toggle Lagrange Points Overlay |
| **`Shift + O`** | Toggle Habitable Zone Overlay |
| **`F1`** / **`Shift + F1`** | Load Solar MMSN / Load Disk Genesis (Organic Planet Formation) |
| **`F2`** - **`F7`**, **`F9`**, **`F11`** | Load scenario presets (TRAPPIST, Kepler-16, Hot Jupiter, Rogue Planet, Little Red Dot, Pulsar, Magnetar, Kozai Triple) |
| **`F10`** | Toggle Telemetry & Habitability Graph Drawer |
| **`F11`** | Toggle Deep-Time Geological Epoch Scrubber (when planet focused) |
| **`F12`** | Quick Save simulation state (`saves/quicksave.json`) |
| **`Shift + F12`** | Quick Load simulation state (`saves/quicksave.json`) |
| **`M`** | Trigger Theia giant impact & Moon formation (exclusive to Solar scenario) |
| **`G`** | Trigger Late Heavy Bombardment cometary cascade (exclusive to Solar scenario) |
| **`F8`** | Hot-swap between GPU compute particles & CPU fallback |

### Live Body Editing (Selected Body)
| Key / Input | Action |
| :--- | :--- |
| **`U`** / **`+`** | Increase mass by $+25\%$ |
| **`J`** / **`-`** | Decrease mass by $-20\%$ |
| **`O`** | Expand orbit by $+10\%$ (re-circularizes) |
| **`L`** | Contract orbit by $-10\%$ (re-circularizes) |
| **`C`** | Cycle composition (Silicate $\to$ Ice $\to$ Iron $\to$ Gas) |
| **`B`** | Apply forward prograde delta-V impulse ($+15\%$) |
| **`K`** | Apply retrograde braking delta-V impulse ($-15\%$) |
| **`I`** | Ignite protostar into Main Sequence / Trigger Coronal Mass Ejection |
| **`Z`** | Zero orbital inclination and re-circularize orbit |
| **`Delete`** / **`Backspace`** | Despawn body cleanly |
| **`X`** | Vaporize body into expanding debris |

---

## 🏗️ Project Architecture

Protostellar is designed around Bevy's data-driven **Entity Component System (ECS)** and modular astrophysical subsystems:

```
protostellar/
├── assets/
│   └── shaders/
│       ├── supernova.wgsl        # Ray-marched volumetric supernova fireball, shock fronts & ionization shells
│       ├── skybox.wgsl           # Milky Way & Cosmic Web procedural skybox with gravitational lensing
│       ├── planet.wgsl           # PBR crusts, spectral exoplanet flora, bioluminescence, ring shadows & eclipses
│       ├── black_hole_disk.wgsl  # Relativistic MHD accretion disk with Doppler beaming & Kerr photon ring
│       ├── nbody_galaxy.wgsl     # WGPU N-body spiral galaxy & NFW dark matter halo compute shader
│       ├── gmc_far_field.wgsl    # Tile-shared GPU far-field gravity compute shader for GMC star clusters
│       ├── atmosphere.wgsl       # Multi-layer Rayleigh/Mie atmospheric scattering & twilight ring
│       ├── relativistic_jet.wgsl # Relativistic polar jet beaming, Doppler flaring & shock knots
│       ├── gas_cloud.wgsl        # 15-layer flared 3D protoplanetary gas disk
│       ├── planetary_rings.wgsl  # Optical depth and micro-ringlets shader
│       ├── particle_orbit.wgsl   # WebGPU compute shader for 100k particles
│       └── particle_render.wgsl  # GPU instanced particle swarm rendering
├── src/
│   ├── gpu/                      # WebGPU compute nodes (SPH gas, N-body galaxy, GMC far-field, collapse)
│   ├── simulation/
│   │   ├── accretion/            # Collision regimes, Theia giant impact, impact melt basins
│   │   ├── atmosphere_escape/    # Hydrodynamic EUV escape & cometary ion tails
│   │   ├── biology/              # Habitability tracking, spectral flora adaptation & technosignature evolution
│   │   ├── components/           # ECS definitions (Mass, Position, Climate, BiosphereState, SpinState, etc.)
│   │   ├── disk/                 # Protoplanetary disk structure, gas drag, pebble drift
│   │   ├── geology/              # Deep-time epochs, continental drift, Great Oxidation Event
│   │   ├── kozai_lidov/          # Hierarchical triple resonance & secular orbital tilt exchange
│   │   ├── physics/              # Modular symplectic leapfrog N-body integrator & forces
│   │   ├── predictor.rs          # Real-time Keplerian trajectory predictor & encounter forecaster
│   │   ├── probes/               # Autonomous Orbiters, geodesic surface Rovers, 3D trails & Raman assays
│   │   ├── relativity/           # 1PN precession, GW Peters orbital decay, relativistic jets & TDEs
│   │   ├── scenarios/            # 16 scenario presets (Solar, Genesis, TRAPPIST, Kepler-16, Sag A*, Galaxy, etc.)
│   │   ├── serialization.rs      # System JSON save/load serializer
│   │   ├── space_weather/        # CMEs, stellar flares, magnetospheric stand-off & auroral ovals
│   │   ├── telemetry.rs          # Symplectic energy auditing, metrics history & CSV export
│   │   ├── terraforming/         # Targeted bombardment spawner & dynamic climate modification
│   │   ├── thermodynamics/       # Stellar evolution, habitability indexing & multi-zone climate regimes
│   │   └── tides/                # Viscoelastic tidal heating, volcanism & spin-orbit locking
│   ├── rendering/
│   │   ├── bodies/               # Meshes, atmospheres, relativistic jets, ring systems & shader uniform sync
│   │   ├── effects/              # Supernovae, Kilonovae, SNR nebulae, Dyson swarms, dark matter halos & conics
│   │   ├── materials.rs          # Custom Bevy PBR & volumetric shader materials
│   │   └── camera.rs             # Pan-orbit camera, zoom, target tracking
│   ├── game/
│   │   ├── ui/                   # Full HUD suite: Inspector, Builder, Scrubber, Telemetry, Trophies, Quick Bar
│   │   ├── interaction.rs        # Keyboard shortcuts, selection, live editing
│   │   ├── slingshot.rs          # Interactive orbital slingshot launcher
│   │   └── time_control.rs       # Time warp, pause, speed stepping
│   └── utils/                    # Astronomical constants, Kepler solvers, state vector conversion
└── tests/
    ├── simulation_tests.rs       # Master integration test harness
    └── simulation_tests/         # 52 specialized test suites covering 440 automated tests
```

---

## 🧪 Automated Test Suite (440 Tests)

Protostellar enforces rigorous physical validity through **440 comprehensive integration tests** across 52 specialized test suites:

- **Surface Horizon Navigation, 3D Terrains & Rover Drive Mechanics**: Tangent horizon camera pose stabilization, surface altitude limits, collision avoidance, and terrain-skimming driving controls.
- **Galilean Moons, Volcanism & Cryotectonics**: Io sulfur volcanism, Europa/Enceladus cryotectonic lineae, cryovolcanic planetary ring replenishment, and multi-moon non-overlapping orbital initialization.
- **Cosmic Hazards & Targeted Cometary Delivery**: Swept-closest-approach collision detection, proportional navigation intercept, guaranteed LHB ocean condensation, and water delivery budgets.
- **Physical Composition & Super-Puff Planetary Realism**: Composition-driven gas giant / sub-Neptune envelope classification, neutral Blinn-Phong dielectric specular optics, and eclipse multi-shadow projection.
- **Exoplanet Biospheres, Spectral Flora & Rover Surface Exploration**: Host-star spectral flora pigment adaptation (`SolarChlorophyll`, `RedDwarfCrimson`, `OrangeDwarfCarotenoid`, `HighUvIndigo`), bioluminescent twilight/nightside canopies, autonomous rover geodesic surface traversal, breadcrumb trail recording, and Raman core-drilling sample collection.
- **GPU Far-Field GMC Gravity & N-Body Galaxies**: Tile-shared GPU far-field gravitational softening, CPU/GPU parity within $10^{-4}$, NFW dark matter halo rotation curves, and Jeans collapse star formation.
- **Supernova Core-Collapse & Persistent Remnant Nebulae**: Mass-differentiated explosion regimes (Hypernova, Type II, Type Ia, Planetary Nebula), prompt breakout fireball decay, high-velocity ejecta kinematics, SPH disk blast clearing, handoff to persistent Sedov-Taylor nebulae ($R(t) \propto t^{0.38}$), multi-element filamentary cooling, and Pulsar Wind Nebula (PWN) synchrotron cores.
- **Kilonova Fireballs & Relativistic Remnants**: Peters GW orbital inspiral coalescence, relativistic short-GRB polar jets, $r$-process lanthanide fireball radioactive cooling, BBH spacetime metric chirp ripples, Kerr QNM ringdown shimmer, and Schwarzschild black hole event horizon scaling ($r_{\text{vis}} \propto 2GM/c^2$) with grazing photon ring lensing.
- **Pristine Disk Genesis & Star Ignition**: Organic coagulation from 100,000 SPH particles, water-ice snow line trapping, protostellar core contraction, and automatic 10.0 MK fusion ignition.
- **Hierarchical Kozai-Lidov Triples**: Secular quadrupole angular momentum conservation ($L_z = \sqrt{1 - e^2} \cos i = \text{const}$), cyclic eccentricity-inclination coupling, and grazing tidal captures.
- **Orbital Mechanics & Conservation**: Symplectic leapfrog energy conservation, Keplerian solver accuracy, high-warp orbital stability.
- **Accretion & Theia Giant Impact**: Oblique impact kinematics, prograde lunar coalescence, Earth spin-up, LLSVP density contrast.
- **Geology & Climate**: Magma ocean cooling, Great Oxidation Event transitions, continental drift phases, vegetation expansion.
- **Relativity & High-Energy**: 1PN perihelion advance, Peters GW inspiral, relativistic velocity bounds ($\beta < 1.0$), Doppler boosting factors, synchrotron emissivity.
- **Tides & Rheology**: Viscoelastic dissipation scaling, 1:1 spin synchronization, tidal heating flux.
- **Atmospheric Physics**: Multi-layer Rayleigh/Mie scattering, photoevaporative mass loss, cometary ion tail aberration.
- **Shadows & Eclipses**: Concurrent 4-moon umbral/penumbral eclipse ratios, planetary ring day/night hemisphere shadowing.
- **Space Weather & Auroral Ovals**: Stellar flares, CME shockwave propagation, planetary magnetosphere stand-off, auroral oval geometry.
- **Trajectory Forecasts**: Bound elliptical vs hyperbolic conics, Hill sphere entries, Roche disruption predictions.
- **UI & System Serialization**: JSON round-trip save/load, quick bar zoning, inspector formatting, HUD minimization, and scenario contextual filtering.

To run all automated tests:
```bash
cargo test --test simulation_tests
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

> **Note:** Always compile with `--release`! Protostellar's numerical integrators, GPU compute pipeline, and 100,000-particle swarms are heavily optimized for release builds, running locked at 120+ FPS on Apple Silicon M-series chips and modern dedicated GPUs.

---

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
