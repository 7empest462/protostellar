# PROTOSTELLAR 🪐✨

**Protostellar** is a high-performance, visually stunning Solar System and Exoplanet Formation Simulator built in Rust using the [Bevy Engine](https://bevyengine.org/). It combines rigorous astrophysical equations of motion with real-time procedural GPU shaders, allowing you to witness 100,000+ particles organically coalesce into stars, rocky worlds, ringed gas giants, relativistic remnants, and intricate moon systems over millions of simulated years.

---

## 📖 Table of Contents
1. [🌟 Recent Additions & Major Engines](#-recent-additions--major-engines)
2. [🎮 Flight Manual & How to Access Everything In-Game](#-flight-manual--how-to-access-everything-in-game)
   - [Camera & Viewport Controls](#camera--viewport-controls)
   - [Time Flow Engine](#time-flow-engine)
   - [Supernova Core-Collapse, Matter Ejection & Remnant Formation](#supernova-core-collapse-matter-ejection--remnant-formation)
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
6. [🧪 Automated Test Suite (312 Tests)](#-automated-test-suite-312-tests)
7. [⚡ Getting Started](#-getting-started)
8. [📜 License](#-license)

---

## 🌟 Recent Additions & Major Engines

Protostellar has evolved far beyond an N-body gravity toy into a comprehensive astrophysical sandbox. Here is a summary of all recent major systems:

| System / Feature | Description | Primary In-Game Access |
| :--- | :--- | :---: |
| **Supernova Core-Collapse & Ejecta Simulation** | High-energy stellar detonations with prompt breakout fireballs, supersonic multi-layer shockwaves, turbulent Rayleigh-Taylor matter clumps, SPH circumstellar disk clearing ($+15\text{ AU/s}$), and asteroid vaporization. Differentiated by progenitor mass: Hypernova / Collapsar ($\ge 25\ M_\odot \to$ Black Hole), Type II ($8 - 25\ M_\odot \to$ Pulsar), Type Ia ($> 1.44\ M_\odot \to$ Complete Disruption), and Planetary Nebula ($0.5 - 8\ M_\odot \to$ White Dwarf). | Open Inspector (**`[I]`**) $\to$ click **`[💥 Supernova]`**, or press **`[N]`** on star, or over-accrete a White Dwarf |
| **Pristine Disk Genesis & Star Ignition** | Class II T-Tauri disk without starter planets. Features 100,000 SPH viscous particles, aerodynamic gas drag, $2.7\text{ AU}$ water snow line trap, protostellar gravitational contraction, and automatic thermonuclear ignition at $10.0\text{ MK}$ ($100\%$) with solar wind clearing. | Press **`[Shift + F1]`** or click **`[Disk Genesis]`** in Scenarios Bar |
| **Secular Kozai-Lidov Resonance** | Quadrupole secular gravitational coupling in hierarchical triples, driving massive cyclic exchanges of orbital inclination and eccentricity ($e \leftrightarrow i$) up to $e \to 0.88+$, triggering extreme tidal heating, captures, or Roche shredding. | Press **`[F11]`** or click **`[HD 80606]`** in Scenarios Bar |
| **Contextual Scenario-Specific HUD** | Dynamic toolbar and telemetry filtering that correlates HUD action menus, tools, and readouts to the currently loaded scenario, eliminating visual clutter while preserving universal sandbox tools everywhere. | Automatic across all 11 scenario presets |
| **Interactive Orbital Slingshot Launcher** | Aim and inject projectiles, comets, or rogue planets with click-and-drag flight vectors. | Press **`[K]`** or click **`[🎯 Slingshot [K]]`** |
| **Trajectory Predictor & Encounter Forecaster** | Conic Keplerian trajectory propagation forecasting closest approaches, Hill sphere entries, and direct impacts. | Press **`[N]`** or click **`[🎯 Forecast: ON [N]]`** |
| **Deep-Time Geological Epoch Scrubber** | Scrub planets across 4.56 Gyr of planetary evolution: Hadean magma oceans, Archean Great Oxidation, supercontinents, and biosphere expansion. | Press **`[F11]`** or click **`[⏳ Epochs [F11]]`** |
| **Relativistic Polar Jets & Synchrotron Cones** | Relativistic beaming ($\theta_{\text{beam}} = 1/\Gamma$), real-time camera Doppler boosting ($D = \delta^{3+\alpha}$), braided helical magnetic flux ropes, and shock knots. | Load **`[F7]`** Pulsar, **`[F9]`** Magnetar, or **`[F6]`** Quasi-Star |
| **Targeted Terraforming & Guided Bombardment** | Precision-targeted projectile launcher delivering water oceans via icy comets, greenhouse breakout via carbonaceous chondrites, or dynamo activation. | Open Inspector (**`[I]`**) $\to$ Terraforming section buttons |
| **Viscoelastic Tidal Heating & Volcanism** | Models tidal dissipation (Kaula/Efroimsky formulations), Io-like volcanism, surface lava lakes, and 1:1 spin-orbit tidal locking. | Open Inspector (**`[I]`**) $\to$ click **`[🌊 Tidal Lock]`** |
| **Multi-Moon Solar Eclipses & Ring Shadows** | Dynamic ray-marched umbra/penumbra solar eclipse shadow discs on planet surfaces and Cassini-divided ring shadows across day/night sides. | Automatic in all systems with moons or rings (e.g. Saturn, Earth-Moon) |
| **Atmospheric Photoevaporation & EUV Winds** | Energy-limited photoevaporative hydrodynamic escape stripping close-in envelopes down to bare chthonian cores with anti-stellar cometary ion tails. | Open Inspector (**`[I]`**) $\to$ click **`[💨 Strip Atm]`** |
| **General Relativistic 1PN & GW Inspiral** | First Post-Newtonian (1PN) perihelion precession (Mercury) and Peters gravitational-wave orbital decay leading to ISCO coalescence and chirp mergers. | Open Inspector (**`[I]`**) $\to$ click **`[🌀 Inspiral]`** |
| **System Save / Load & Scenario Serializer** | Full JSON serialization preserving masses, compositions, climates, spins, rings, basins, tides, and orbital vectors. | Press **`[F12]`** (Save) / **`[Shift+F12]`** (Load) |
| **Scrollable Target Inspector & Universal HUD Pills** | Clean, organized inspection panel with vertical scroll, categorized sections, and universal `🗕` minimization with glowing restore pills. | Press **`[I]`** or click any corner `🗕` button |

---

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
  - Over-accrete a White Dwarf past the Chandrasekhar limit ($1.44\ M_\odot$) or a Neutron Star past the TOV limit ($2.17\ M_\odot$).

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
│       ├── skybox.wgsl           # Milky Way & Cosmic Web procedural skybox with gravitational lensing
│       ├── planet.wgsl           # PBR crusts, magma oceans, Ray-marched ring shadows & eclipses
│       ├── atmosphere.wgsl       # Multi-layer Rayleigh/Mie atmospheric scattering & twilight ring
│       ├── relativistic_jet.wgsl # Relativistic polar jet beaming, Doppler flaring & shock knots
│       ├── gas_cloud.wgsl        # 15-layer flared 3D protoplanetary gas disk
│       ├── planetary_rings.wgsl  # Optical depth and micro-ringlets shader
│       ├── particle_orbit.wgsl   # WebGPU compute shader for 100k particles
│       └── particle_render.wgsl  # GPU instanced particle swarm rendering
├── src/
│   ├── gpu/                      # WebGPU compute nodes & async double-buffered staging
│   ├── simulation/
│   │   ├── accretion/            # Collision regimes, Theia giant impact, impact melt basins
│   │   ├── atmosphere_escape/    # Hydrodynamic EUV escape & cometary ion tails
│   │   ├── components/           # ECS definitions (Mass, Position, Climate, JetState, SpinState, etc.)
│   │   ├── disk/                 # Protoplanetary disk structure, gas drag, pebble drift
│   │   ├── geology/              # Deep-time epochs, continental drift, Great Oxidation Event
│   │   ├── kozai_lidov/          # Hierarchical triple resonance & secular orbital tilt exchange
│   │   ├── predictor.rs          # Real-time Keplerian trajectory predictor & encounter forecaster
│   │   ├── relativity/           # 1PN precession, GW Peters orbital decay, relativistic jets
│   │   ├── scenarios/            # 11 scenario presets (Solar, Genesis, TRAPPIST, Kepler-16, Pulsar, Magnetar, etc.)
│   │   ├── serialization.rs      # System JSON save/load serializer
│   │   ├── space_weather/        # CMEs, stellar flares, magnetospheric stand-off & auroral ovals
│   │   ├── telemetry.rs          # Symplectic energy auditing, metrics history & CSV export
│   │   ├── terraforming/         # Targeted bombardment spawner & dynamic climate modification
│   │   ├── thermodynamics.rs     # Stellar evolution, habitability indexing & climate regimes
│   │   ├── tides/                # Viscoelastic tidal heating, volcanism & spin-orbit locking
│   │   └── physics.rs            # Symplectic leapfrog N-body integrator
│   ├── rendering/
│   │   ├── bodies/               # Meshes, atmospheres, relativistic jets, ring systems
│   │   ├── effects/              # Supernova core-collapse explosions, conics, orbit ribbons, cometary tails
│   │   ├── materials.rs          # Custom Bevy PBR & volumetric shader materials
│   │   └── camera.rs             # Pan-orbit camera, zoom, target tracking
│   ├── game/
│   │   ├── ui/                   # Full HUD suite: Inspector, Builder, Scrubber, Telemetry, Quick Bar
│   │   ├── interaction.rs        # Keyboard shortcuts, selection, live editing
│   │   ├── slingshot.rs          # Interactive orbital slingshot launcher
│   │   └── time_control.rs       # Time warp, pause, speed stepping
│   └── utils/                    # Astronomical constants, Kepler solvers, state vector conversion
└── tests/
    ├── simulation_tests.rs       # Master integration test harness
    └── simulation_tests/         # 37 specialized test suites covering 312 automated tests
```

---

## 🧪 Automated Test Suite (312 Tests)

Protostellar enforces rigorous physical validity through **312 comprehensive integration tests** across 37 specialized test suites:

- **Supernova Core-Collapse & Ejecta**: Mass-differentiated explosion regimes (Hypernova, Type II, Type Ia, Planetary Nebula), prompt breakout fireball decay, high-velocity ejecta kinematics, SPH disk blast clearing, and planet atmospheric escape tails.
- **Pristine Disk Genesis & Star Ignition**: Organic coagulation from 100,000 SPH particles, water-ice snow line trapping, protostellar core contraction, and automatic 10.0 MK fusion ignition.
- **Hierarchical Kozai-Lidov Triples**: Secular quadrupole angular momentum conservation ($L_z = \sqrt{1 - e^2} \cos i = \text{const}$), cyclic eccentricity-inclination coupling, and grazing tidal captures.
- **Orbital Mechanics & Conservation**: Symplectic leapfrog energy conservation, Keplerian solver accuracy, high-warp orbital stability.
- **Accretion & Theia Giant Impact**: Oblique impact kinematics, prograde lunar coalescence, Earth spin-up, LLSVP density contrast.
- **Geology & Climate**: Magma ocean cooling, Great Oxidation Event transitions, continental drift phases, vegetation expansion.
- **Relativity & High-Energy**: 1PN perihelion advance, Peters GW inspiral, relativistic velocity bounds ($\beta < 1.0$), Doppler boosting factors, synchrotron emissivity.
- **Tides & Rheology**: Viscoelastic dissipation scaling, 1:1 spin synchronization, tidal heating flux.
- **Atmospheric Physics**: Multi-layer Rayleigh/Mie scattering, photoevaporative mass loss, cometary ion tail aberration.
- **Shadows & Eclipses**: Umbral/penumbral eclipse ratios, planetary ring day/night hemisphere shadowing.
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
