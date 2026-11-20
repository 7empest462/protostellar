use crate::simulation::geology::types::EpochTargetPlanet;

use super::types::{InspectorTab, UiButtonAction};

impl UiButtonAction {
    /// Returns the HUD hover tooltip text associated with this UI button action.
    pub fn tooltip_description(&self) -> &'static str {
        self.time_and_scenario_tooltip()
            .or_else(|| self.selector_and_builder_tooltip())
            .or_else(|| self.panels_and_geology_tooltip())
            .or_else(|| self.view_and_body_manipulation_tooltip())
            .or_else(|| self.biosphere_and_volatiles_tooltip())
            .or_else(|| self.chrome_saves_and_inspector_tooltip())
            .unwrap_or("")
    }

    fn time_and_scenario_tooltip(&self) -> Option<&'static str> {
        Some(match self {
            UiButtonAction::TimePause => {
                "[Space]: Pause or resume continuous orbital physics flow."
            }
            UiButtonAction::TimeSpeedRealtime => "[1]: 1:1 Real-time progression (1s = 1s).",
            UiButtonAction::TimeSpeed1Min => {
                "[9] or [S-1]: 1 second = 1 minute simulation time (60x real-time)."
            }
            UiButtonAction::TimeSpeed5Min => {
                "[0] or [S-2]: 1 second = 5 minutes simulation time (300x real-time)."
            }
            UiButtonAction::TimeSpeed6Hr => {
                "[S-3]: 1 second = 6 hours simulation time (21,600x real-time)."
            }
            UiButtonAction::TimeSpeed12Hr => {
                "[S-4]: 1 second = 12 hours simulation time (43,200x real-time)."
            }
            UiButtonAction::TimeSpeed24Hr => {
                "[S-5]: 1 second = 24 hours / 1 day simulation time (86,400x real-time)."
            }
            UiButtonAction::TimeSpeed72Hr => {
                "[S-6]: 1 second = 72 hours / 3 days simulation time (259,200x real-time)."
            }
            UiButtonAction::TimeSpeed1 => "[2]: 1.0x Orbital flow speed (1s = 11.0 days).",
            UiButtonAction::TimeSpeed100 => "[4]: 100x Accelerated time progression (~3.0 yr/sec).",
            UiButtonAction::TimeSpeed10k => {
                "[6]: 10,000x High-speed planetary accretion flow (~300 yr/sec)."
            }
            UiButtonAction::TimeSpeed1M => {
                "[8]: 1,000,000x Deep astronomical time-warp (~30 kyr/sec)."
            }
            UiButtonAction::LoadScenarioSolar => {
                "[F1]: Reset to standard 4.5 Gyr Hayashi Solar Nebula MMSN with central protostar and 10 embryos."
            }
            UiButtonAction::LoadScenarioGenesis => {
                "[S-F1]: Load Protoplanetary Disk Genesis: SPH viscous gas, aerodynamic drag, snow line trap & organic planet formation."
            }
            UiButtonAction::LoadScenarioTrappist => {
                "[F2]: Load TRAPPIST-1 ultracool red dwarf system with 7 resonant Earths (3 in Habitable Zone)."
            }
            UiButtonAction::LoadScenarioKepler16 => {
                "[F3]: Load Kepler-16 'Tatooine' circumbinary system with K/M binary star pair and giant planet."
            }
            UiButtonAction::LoadScenarioHotJupiter => {
                "[F4]: Load Hot Jupiter inward migration scenario (Type II disk migration from 5.2 AU -> 0.045 AU)."
            }
            UiButtonAction::LoadScenarioRoguePlanet => {
                "[F5]: Load Rogue Planet Flyby scenario (Hyperbolic 3.5 M_Jup interloper scattering the solar system)."
            }
            UiButtonAction::LoadScenarioLittleRedDot => {
                "[F6]: Load JWST Little Red Dot (100,000 M☉ Black Hole Star encased in 60 AU hydrogen cocoon)."
            }
            UiButtonAction::LoadScenarioPulsar => {
                "[F7]: Load PSR B1257+12 (Millisecond pulsar Lich with 3 confirmed zombie exoplanets)."
            }
            UiButtonAction::LoadScenarioMagnetar => {
                "[F9]: Load SGR 1806-20 (Ultra-magnetized 10¹⁵ G magnetar with starquake flares & companion)."
            }
            UiButtonAction::LoadScenarioRelativisticBinary => {
                "[F10]: Load PSR B1913+16 Hulse-Taylor relativistic binary pulsar and gravitational waves."
            }
            UiButtonAction::LoadScenarioKozaiTriple => {
                "[F11]: Load HD 80606 Kozai-Lidov hierarchical triple secular resonance and tidal migration."
            }
            UiButtonAction::LoadScenarioSagittariusAStar => {
                "[F10]: Load Sagittarius A* supermassive black hole (4.3×10⁶ M☉) with relativistic S-star cluster."
            }
            UiButtonAction::LoadScenarioMolecularCloud => {
                "Load Giant Molecular Cloud (24 M☉) Jeans instability cluster collapse with 10 protostellar seeds."
            }
            UiButtonAction::LoadScenarioGalacticCollision => {
                "Load Milkomeda Galactic Collision: N-Body Galaxy merger with Dark Matter Halos and 200,000 stars."
            }
            _ => return None,
        })
    }

    fn selector_and_builder_tooltip(&self) -> Option<&'static str> {
        Some(match self {
            UiButtonAction::SelectEntity(_) => "Focus and inspect this celestial body.",
            UiButtonAction::SelectStar => {
                "Select the central star to inspect solar mass, temperature, and corona."
            }
            UiButtonAction::SelectMercury => "Select the innermost rocky terrestrial planet.",
            UiButtonAction::SelectEarth => "Select the habitable-zone terrestrial planet.",
            UiButtonAction::SelectJupiter => "Select the dominant outer gas giant planet.",
            UiButtonAction::SelectKuiper => "Select the outermost icy Kuiper belt planetesimal.",
            UiButtonAction::CycleTarget => {
                "[Tab]: Cycle camera focus through all active celestial bodies."
            }
            UiButtonAction::ToggleMinimizeQuickBar => {
                "[H]: Minimize or expand the top celestial body shortcut bar."
            }
            UiButtonAction::ToggleMinorBodies => {
                "Toggle visibility of minor asteroids and planetesimals."
            }
            UiButtonAction::ToggleBelt(zone) => zone.title(),
            UiButtonAction::ExpandAllBelts => {
                "Expand all astronomical belt groups in the quick selector."
            }
            UiButtonAction::CollapseAllBelts => {
                "Contract all astronomical belt groups into compact summary counts."
            }
            UiButtonAction::ToggleEmbryos => "Toggle visibility of protoplanetary embryos.",
            UiButtonAction::SpawnOortCloudSwarm => {
                "[Shift+O]: Procedurally populate the isotropic Hills (200-2,000 AU) and Outer Oort (2,000-25,000 AU) cometary swarm."
            }
            UiButtonAction::TogglePlanetBuilder => {
                "[P]: Open or close the interactive Planet Builder & Spawner panel."
            }
            UiButtonAction::BuilderSelectPreset(_) => {
                "Load this physical planetary archetype preset into the Planet Builder."
            }
            UiButtonAction::BuilderMassStep(s) => {
                if *s > 0 {
                    "Scale up target world mass."
                } else {
                    "Scale down target world mass."
                }
            }
            UiButtonAction::BuilderDistanceStep(s) => {
                if *s > 0 {
                    "Increase target orbital semi-major axis (radius in AU)."
                } else {
                    "Decrease target orbital semi-major axis (radius in AU)."
                }
            }
            UiButtonAction::BuilderCycleEccentricity => {
                "Cycle orbital eccentricity between Circular (0.00), Moderate (0.15), High (0.60), and Hyperbolic (1.25)."
            }
            UiButtonAction::BuilderCycleComposition => {
                "Cycle composition fractions (Rocky, Oceanic Ice, Metal Core, Gas Giant)."
            }
            UiButtonAction::BuilderToggleClickSpawn => {
                "Toggle 3D plane click-to-place mode (click disk to place world)."
            }
            UiButtonAction::BuilderExecuteSpawn => {
                "Spawn the configured celestial world into orbit immediately!"
            }
            UiButtonAction::SpawnSubRocheMoon => {
                "Spawn a volatile-rich moon directly inside the selected planet's fluid Roche limit to trigger tidal shredding and ring creation!"
            }
            _ => return None,
        })
    }

    fn panels_and_geology_tooltip(&self) -> Option<&'static str> {
        Some(match self {
            UiButtonAction::ToggleTelemetryPanel => {
                "[F10]: Open or close the planetary climate & habitability telemetry graphing panel."
            }
            UiButtonAction::ToggleTrophiesPanel => {
                "🏆 Trophies: View unlocked scientific milestones and scenario achievements."
            }
            UiButtonAction::ToggleTechTreePanel => {
                "🔬 Tech Tree [Shift-T]: Research advanced propulsion, terraforming tools, and megastructures."
            }
            UiButtonAction::ToggleCampaignPanel => {
                "🎯 Campaign [Shift-M]: Goal-oriented astrophysics missions and scenario challenges."
            }
            UiButtonAction::ToggleCampaignTracker => {
                "Minimize or expand the on-screen campaign mission objectives tracker."
            }
            UiButtonAction::StartCampaignMission(_) => {
                "Launch this goal-oriented scenario challenge."
            }
            UiButtonAction::ClaimCampaignReward => {
                "Claim Science Data bounty for completing the campaign mission."
            }
            UiButtonAction::AbandonCampaignMission => {
                "Abort active campaign mission and return to sandbox."
            }
            UiButtonAction::ResearchTech(_) => {
                "Research and unlock this advanced scientific capability using accumulated Science Data."
            }
            UiButtonAction::ToggleHazardsPanel => {
                "⚡ Hazards [Shift-H]: Trigger deep-space cosmic hazards and test planetary defenses."
            }
            UiButtonAction::TriggerCosmicHazard(h) => h.title(),
            UiButtonAction::ToggleAutoHazards => {
                "Toggle stochastic background generation of deep-space cosmic hazards."
            }
            UiButtonAction::CycleTonemappingMode => {
                "🎨 Tonemapping [Shift-C]: Cycle screen post-processing display curves (AgX -> ACES -> Reinhard)."
            }
            UiButtonAction::SelectTelemetryMetric(_) => {
                "Switch active telemetry graph metric (Habitability, Temperature, Ocean, Pressure, Orbit)."
            }
            UiButtonAction::ExportTelemetryCsv => {
                "Export current planetary telemetry history to a standard CSV file in ./exports/."
            }
            UiButtonAction::ToggleEpochScrubberPanel => {
                "[F11]: Open or close the deep geological epoch scrubber and continental drift timeline."
            }
            UiButtonAction::SelectEpochTargetPlanet(planet) => match planet {
                EpochTargetPlanet::Earth => {
                    "Switch epoch scrubber to Earth's 4.56 Gyr geological history."
                }
                EpochTargetPlanet::Mars => {
                    "Switch epoch scrubber to Mars's Noachian & Amazonian planetary history."
                }
                EpochTargetPlanet::Venus => {
                    "Switch epoch scrubber to Venus's temperate ocean & runaway greenhouse history."
                }
            },
            UiButtonAction::ScrubToEpoch(epoch) => epoch.supercontinent_name(),
            UiButtonAction::ScrubTimeStep(myr) => {
                if *myr > 0 {
                    "Advance geological timeline into the future."
                } else {
                    "Step geological timeline backwards in deep time."
                }
            }
            UiButtonAction::ToggleScrubAutoAdvance => {
                "Toggle continuous simulation time auto-advance for the geological clock."
            }
            _ => return None,
        })
    }

    fn view_and_body_manipulation_tooltip(&self) -> Option<&'static str> {
        Some(match self {
            UiButtonAction::ToggleOrbitMode => {
                "[Y]: Cycle orbit trails (All -> Selected Target Only -> Hidden)."
            }
            UiButtonAction::CycleOverlayMode => {
                "[V]: Cycle diagnostic HUD overlays (Natural Color -> Spectral Temperature -> Hill Spheres & Gaps)."
            }
            UiButtonAction::CycleParticleDensity => {
                "Cycle GPU particle swarm density preset (100k Standard -> 250k High -> 500k Ultra)."
            }
            UiButtonAction::ToggleTractor => {
                "[T]: Toggles Gravitational Tractor Beam to pull particles & planetesimals."
            }
            UiButtonAction::ToggleTrajectoryPredictor => {
                "[N]: Toggle real-time trajectory prediction and close-encounter / impact warning reticles."
            }
            UiButtonAction::ToggleLagrangePoints => {
                "[Shift-L]: Toggle gravitational Lagrange libration points (L1-L5) and Hill sphere wireframe."
            }
            UiButtonAction::ToggleHabitableZone => {
                "[Shift-Z]: Toggle circumstellar Habitable Zone (Conservative & Optimistic) and Water Ice Frost Line."
            }
            UiButtonAction::ToggleAtmospheres => {
                "[Shift-A]: Strip or restore planetary atmospheres and gas giant envelopes to inspect rocky/metallic/icy interiors beneath."
            }
            UiButtonAction::ToggleClouds => {
                "[Shift-W]: Hide or restore cloud decks while keeping planetary atmospheres, Rayleigh skies, and deep gas bands visible."
            }
            UiButtonAction::ToggleHrDiagram => {
                "[Shift-R]: Open or close the interactive Hertzsprung–Russell Stellar Evolution Diagram window."
            }
            UiButtonAction::ToggleMinimizeHrDiagram => {
                "Minimize or restore the Hertzsprung–Russell Stellar Evolution panel."
            }
            UiButtonAction::IncreaseMass => "[U / Shift+=]: Accrete +25% mass into selected body.",
            UiButtonAction::DecreaseMass => {
                "[J / Shift+-]: Strip -20% outer envelope mass from selected body."
            }
            UiButtonAction::ExpandOrbit => {
                "[O]: Boost orbital energy to expand orbital radius +10%."
            }
            UiButtonAction::ContractOrbit => {
                "[L]: Brake orbital energy to contract orbital radius -10%."
            }
            UiButtonAction::CycleComposition => {
                "[C]: Cycle composition between Rocky, Metallic, Icy, and Volatile."
            }
            UiButtonAction::BoostDeltaV => "[B]: Prograde orbital velocity acceleration boost.",
            UiButtonAction::BrakeDeltaV => "[Shift-B]: Retrograde orbital velocity braking burn.",
            UiButtonAction::InjectEmbryo => {
                "[Alt-M]: Trigger Theia-Earth Moon-forming giant impact / intercept trajectory."
            }
            UiButtonAction::VaporizeBody => {
                "[Del]: Shatter selected celestial body into dust & fragments."
            }
            UiButtonAction::FocusLock => "[F]: Focus & track camera onto selected celestial body.",
            UiButtonAction::ToggleSurfaceHorizonCamera => {
                "[Shift+V]: Toggle First-Person Rover / Planetary Surface Horizon Camera (WASD to drive, R-Drag to look 360°)."
            }
            UiButtonAction::ToggleInternalCutaway => {
                "[Shift+X]: Toggle 3D internal planetary cutaway cross-section & core geodynamics."
            }
            UiButtonAction::ResetView => "[R]: Reset camera to overview orientation.",
            UiButtonAction::DeselectBody => "[Esc]: Close inspector and clear selection.",
            UiButtonAction::FixOrbit => "[Z]: Circularize orbital eccentricity to 0.00.",
            UiButtonAction::IgniteStar => "[I]: Force instant core ignition / coronal solar blast.",
            UiButtonAction::TriggerCoronalMassEjection => {
                "[Shift+I]: Trigger extreme Coronal Mass Ejection (CME) flare & plasma shockwave."
            }
            UiButtonAction::TriggerLhb => {
                "[G]: Trigger 2:1 resonance migration & Late Heavy Bombardment."
            }
            UiButtonAction::ShatterIntoRings => {
                "[X]: Tidally disrupt selected moon/body into glowing planetary rings."
            }
            _ => return None,
        })
    }

    fn biosphere_and_volatiles_tooltip(&self) -> Option<&'static str> {
        Some(match self {
            UiButtonAction::SeedLife => {
                "[E]: Seed primordial water oceans, atmosphere, and photosynthetic biosphere."
            }
            UiButtonAction::AgeStar => {
                "[N]: Step central star forward through far-future lifecycle (Red Giant -> Nebula -> White Dwarf)."
            }
            UiButtonAction::BombardComet => {
                "Launch guided water-rich comet to deliver H2O volatiles & create oceans."
            }
            UiButtonAction::BombardChondrite => {
                "Launch carbonaceous chondrite to deliver N2 and CO2, boosting atmospheric pressure."
            }
            UiButtonAction::BombardSalvo => {
                "Launch aerobraking volatile salvo for gentle atmospheric influx without cratering."
            }
            UiButtonAction::BombardCore => {
                "Launch dense iron-nickel impactor to stimulate core temperature and magnetic dynamo."
            }
            UiButtonAction::AddOceanWater => {
                "[]]: Add surface water (+10% sea level) to expand oceans and submerge continental lowlands."
            }
            UiButtonAction::DrainOceanWater => {
                "[[]: Drain surface water (-10% sea level) to expose continental shelves, ridges, and trenches."
            }
            UiButtonAction::DryBasin => {
                "[Shift+[]: Drain all surface liquid water to expose desiccated oceanic lithosphere and bathymetry."
            }
            UiButtonAction::EarthSeaLevel => {
                "Restore terrestrial sea level to Earth baseline (71% ocean coverage, 1.0 atm)."
            }
            UiButtonAction::TidalLock => {
                "Synchronize planetary rotation with orbital period (1:1 lock) and align spin axis."
            }
            UiButtonAction::AccelerateInspiral => {
                "Accelerate gravitational wave inspiral decay for selected compact binary pair."
            }
            UiButtonAction::StripAtmosphere => {
                "Trigger extreme photoevaporative XUV burst to strip volatile envelope down to bare rocky core."
            }
            UiButtonAction::ToggleSuperEddington => {
                "[X]: Toggle Super-Eddington hyper-accretion onto the central black hole seed."
            }
            UiButtonAction::TriggerBlowoutCocoon => {
                "[B]: Trigger radiation envelope blowout to unveil the naked Supermassive Quasar."
            }
            UiButtonAction::SpawnInfallPop3Star => {
                "[T]: Spawn an infalling Population III hypergiant star to observe a Tidal Disruption Event (TDE)."
            }
            UiButtonAction::TriggerGmcSupernovaShock => {
                "💥 Detonate an external massive star into a Supernova outside the cloud, driving a compressive shockwave."
            }
            _ => return None,
        })
    }

    fn chrome_saves_and_inspector_tooltip(&self) -> Option<&'static str> {
        Some(match self {
            UiButtonAction::ToggleFullScreenHud => {
                "[F11]: Toggle clean full-screen view (hide/show all HUD overlays)."
            }
            UiButtonAction::ToggleTopLeftPanel => {
                "Minimize or expand top-left system statistics panel."
            }
            UiButtonAction::ToggleTopRightPanel => {
                "Minimize or expand top-right diagnostics and time controls."
            }
            UiButtonAction::ToggleInspectorPanel => {
                "Minimize or expand celestial body inspector & action toolbar."
            }
            UiButtonAction::ToggleScenariosPanel => {
                "Open or close the astrophysical sandbox scenario presets drawer [F1–F11]."
            }
            UiButtonAction::ToggleTopControlsPanel => {
                "Minimize or expand top-right view & tool controls card."
            }
            UiButtonAction::ToggleBottomRightPanel => {
                "Minimize or expand bottom-right navigation and keyboard shortcuts panel."
            }
            UiButtonAction::ToggleBottomCenterPanel => {
                "Minimize or expand bottom-center simulation timer and speed controls."
            }
            UiButtonAction::ToggleMinimizePlanetBuilder => {
                "Minimize or expand the interactive Planet Builder drawer."
            }
            UiButtonAction::ToggleMinimizeTelemetryPanel => {
                "Minimize or expand the planetary telemetry & climate graph drawer."
            }
            UiButtonAction::ToggleSlingshotMode => {
                "[K]: Toggle Interactive Orbital Slingshot Launcher (click-and-drag to aim & launch)."
            }
            UiButtonAction::CycleSlingshotArchetype => {
                "[C]: Cycle Slingshot body archetype (Asteroid -> Comet -> Terrestrial -> Water World -> Gas Giant -> Rogue)."
            }
            UiButtonAction::QuickSave => "[F12]: Quick Save solar system state to disk (JSON).",
            UiButtonAction::QuickLoad => {
                "[Shift+F12]: Quick Load saved solar system state from disk."
            }
            UiButtonAction::SaveNamedSlot => {
                "Save current solar system state to the named save slot."
            }
            UiButtonAction::LoadNamedSlot => "Load solar system state from the named save slot.",
            UiButtonAction::CopySystemSeedClipboard => {
                "Copy active system state as a compact seed to the clipboard."
            }
            UiButtonAction::PasteSystemSeedClipboard => {
                "Import and restore system seed from the clipboard."
            }
            UiButtonAction::ToggleSavesPanel => {
                "Open or close the Multi-Save & Scenario Slot Manager."
            }
            UiButtonAction::RefreshSaveSlots => "Rescan the saves/ directory for saved systems.",
            UiButtonAction::SaveNewScenarioSlot => {
                "Create a new numbered save slot tagged for the active scenario."
            }
            UiButtonAction::ToggleSavesScenarioFilter => {
                "Toggle filtering the save list to only the currently active scenario."
            }
            UiButtonAction::LoadSaveSlot(_) => "Load this saved solar system state from disk.",
            UiButtonAction::OverwriteSaveSlot(_) => {
                "Overwrite this save slot with the current solar system state."
            }
            UiButtonAction::SelectSaveSlot(_) => {
                "Load this slot name into the active Save Slot input box."
            }
            UiButtonAction::DeleteSaveSlot(_) => {
                "Delete this save file from disk (click twice to confirm)."
            }
            UiButtonAction::ClearQuickBarFilter => "Clear active quick bar search filter.",
            UiButtonAction::SelectInspectorTab(tab) => match tab {
                InspectorTab::Overview => {
                    "Switch to the Overview inspector tab (orbital parameters, mass, and bulk composition)."
                }
                InspectorTab::GeodynamicsBio => {
                    "Switch to Geodynamics & Biosphere tab (volcanism, tectonics, oceans, and terraforming)."
                }
                InspectorTab::RelativityFields => {
                    "Switch to Fields & Relativity tab (magnetic fields, general relativity, and high-energy physics)."
                }
            },
            UiButtonAction::AdvanceCivilization => {
                "Advance civilization: spawns a high-tech biosphere and activates city lights on the night side."
            }
            UiButtonAction::LaunchProbe => {
                "Launch a Space Probe: sends an Orbiter or Rover from a technologically advanced world (or star) to explore the selected target."
            }
            UiButtonAction::ToggleListSortMode => {
                "Toggle whether the list of celestial bodies is sorted by distance from the central star or by descending mass."
            }
            UiButtonAction::ToggleAudioMute => {
                "[M]: Mute or unmute procedural cosmic sonification audio (muted by default)."
            }
            _ => return None,
        })
    }
}
