//! Archetype presets and interactive planet builder state resource.

use crate::utils::constants::*;
use bevy::prelude::*;

/// Planetary and stellar archetype templates for the Interactive Planet Builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderPreset {
    EarthLike,
    SuperEarth,
    JupiterLike,
    SuperJupiter,
    HeavySuperJupiter,
    BrownDwarf,
    WaterWorld,
    MoltenProtoplanet,
    IceGiant,
    RogueInvader,
    RedDwarfStar,
    OrangeDwarf,
    WolfRayet,
    PopIIIStar,
    QuarkStar,
    DarkStar,
    BosonStar,
    IronStar,
    ThorneZytkow,
    IoLike,
    EnceladusLike,
}

impl BuilderPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            BuilderPreset::EarthLike => "🌍 Earth-like",
            BuilderPreset::SuperEarth => "💎 Super-Earth (3.5 M_E)",
            BuilderPreset::JupiterLike => "🪐 Jupiter-like",
            BuilderPreset::SuperJupiter => "👑 Super-Jup (3.5 M_J)",
            BuilderPreset::HeavySuperJupiter => "🔮 Heavy Super-J (8.0 M_J)",
            BuilderPreset::BrownDwarf => "🟤 Brown Dwarf (30 M_J)",
            BuilderPreset::WaterWorld => "🌊 Water World",
            BuilderPreset::MoltenProtoplanet => "🌋 Molten Protoplanet",
            BuilderPreset::IceGiant => "❄️ Ice Giant",
            BuilderPreset::RogueInvader => "☄️ Rogue Invader",
            BuilderPreset::RedDwarfStar => "☀️ Red Dwarf Star",
            BuilderPreset::OrangeDwarf => "🟠 Orange Dwarf (K-Type)",
            BuilderPreset::WolfRayet => "⚡ Wolf-Rayet (Stripped)",
            BuilderPreset::PopIIIStar => "✨ Pop III (Primordial)",
            BuilderPreset::QuarkStar => "⚛️ Quark Star (Strange)",
            BuilderPreset::DarkStar => "🌑 Dark Star (WIMP)",
            BuilderPreset::BosonStar => "🔮 Boson Star (Soliton)",
            BuilderPreset::IronStar => "⚙️ Iron Star (Degenerate)",
            BuilderPreset::ThorneZytkow => "🩸 Thorne-Żytkow",
            BuilderPreset::IoLike => "🌋 Volcanic (Io)",
            BuilderPreset::EnceladusLike => "❄️ Cryo (Enceladus)",
        }
    }
}

/// Interactive custom planet builder and spawner state.
#[derive(Resource, Debug, Clone)]
pub struct PlanetBuilderState {
    pub is_open: bool,
    pub active_preset: BuilderPreset,
    pub custom_name: String,
    pub mass_solar: f64,
    pub semi_major_axis_au: f64,
    pub eccentricity: f64,
    pub rock_frac: f32,
    pub ice_frac: f32,
    pub metal_frac: f32,
    pub gas_frac: f32,
    pub click_to_spawn_mode: bool,
    pub is_minimized: bool,
}

impl Default for PlanetBuilderState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_preset: BuilderPreset::EarthLike,
            custom_name: "Proto-Earth".to_string(),
            mass_solar: EARTH_MASS_SOLAR * 1.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.016,
            rock_frac: 0.68,
            ice_frac: 0.02,
            metal_frac: 0.30,
            gas_frac: 0.0,
            click_to_spawn_mode: false,
            is_minimized: false,
        }
    }
}

impl PlanetBuilderState {
    pub fn apply_preset(&mut self, preset: BuilderPreset) {
        self.active_preset = preset;
        if !self.apply_planetary_preset(preset) {
            self.apply_stellar_and_exotic_preset(preset);
        }
    }

    fn apply_planetary_preset(&mut self, preset: BuilderPreset) -> bool {
        match preset {
            BuilderPreset::EarthLike => {
                self.custom_name = "New Earth".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 1.0;
                self.semi_major_axis_au = 1.0;
                self.eccentricity = 0.016;
                self.rock_frac = 0.68;
                self.ice_frac = 0.02;
                self.metal_frac = 0.30;
                self.gas_frac = 0.0;
                true
            }
            BuilderPreset::SuperEarth => {
                self.custom_name = "Kepler-SuperEarth".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 3.5;
                self.semi_major_axis_au = 1.35;
                self.eccentricity = 0.025;
                self.rock_frac = 0.65;
                self.ice_frac = 0.03;
                self.metal_frac = 0.30;
                self.gas_frac = 0.02;
                true
            }
            BuilderPreset::JupiterLike => {
                self.custom_name = "Jovian-Prime".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 1.0;
                self.semi_major_axis_au = 5.2;
                self.eccentricity = 0.048;
                self.rock_frac = 0.03;
                self.ice_frac = 0.02;
                self.metal_frac = 0.01;
                self.gas_frac = 0.94;
                true
            }
            BuilderPreset::SuperJupiter => {
                self.custom_name = "Super-Jupiter".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 3.5;
                self.semi_major_axis_au = 3.2;
                self.eccentricity = 0.08;
                self.rock_frac = 0.02;
                self.ice_frac = 0.02;
                self.metal_frac = 0.01;
                self.gas_frac = 0.95;
                true
            }
            BuilderPreset::HeavySuperJupiter => {
                self.custom_name = "Mega-Jovian".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 8.0;
                self.semi_major_axis_au = 9.5;
                self.eccentricity = 0.06;
                self.rock_frac = 0.01;
                self.ice_frac = 0.01;
                self.metal_frac = 0.01;
                self.gas_frac = 0.97;
                true
            }
            BuilderPreset::BrownDwarf => {
                self.custom_name = "Sub-Stellar Dwarf".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 30.0;
                self.semi_major_axis_au = 22.0;
                self.eccentricity = 0.05;
                self.rock_frac = 0.01;
                self.ice_frac = 0.01;
                self.metal_frac = 0.01;
                self.gas_frac = 0.97;
                true
            }
            BuilderPreset::WaterWorld => {
                self.custom_name = "Oceania".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 2.5;
                self.semi_major_axis_au = 1.35;
                self.eccentricity = 0.02;
                self.rock_frac = 0.30;
                self.ice_frac = 0.60;
                self.metal_frac = 0.10;
                self.gas_frac = 0.0;
                true
            }
            BuilderPreset::MoltenProtoplanet => {
                self.custom_name = "Vulcan".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 0.15;
                self.semi_major_axis_au = 0.45;
                self.eccentricity = 0.15;
                self.rock_frac = 0.75;
                self.ice_frac = 0.0;
                self.metal_frac = 0.25;
                self.gas_frac = 0.0;
                true
            }
            BuilderPreset::IceGiant => {
                self.custom_name = "Sub-Neptune".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 15.0;
                self.semi_major_axis_au = 19.2;
                self.eccentricity = 0.04;
                self.rock_frac = 0.20;
                self.ice_frac = 0.65;
                self.metal_frac = 0.05;
                self.gas_frac = 0.10;
                true
            }
            BuilderPreset::RogueInvader => {
                self.custom_name = "Nemesis-Invader".to_string();
                self.mass_solar = JUPITER_MASS_SOLAR * 3.5;
                self.semi_major_axis_au = 35.0;
                self.eccentricity = 1.25;
                self.rock_frac = 0.02;
                self.ice_frac = 0.02;
                self.metal_frac = 0.01;
                self.gas_frac = 0.95;
                true
            }
            BuilderPreset::IoLike => {
                self.custom_name = "Tidal-Io".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 0.015;
                self.semi_major_axis_au = 5.204 + 0.0028;
                self.eccentricity = 0.0041;
                self.rock_frac = 0.85;
                self.ice_frac = 0.0;
                self.metal_frac = 0.15;
                self.gas_frac = 0.0;
                true
            }
            BuilderPreset::EnceladusLike => {
                self.custom_name = "Cryo-Enceladus".to_string();
                self.mass_solar = EARTH_MASS_SOLAR * 0.000_018;
                self.semi_major_axis_au = 9.582 + 0.0016;
                self.eccentricity = 0.0047;
                self.rock_frac = 0.06;
                self.ice_frac = 0.92;
                self.metal_frac = 0.02;
                self.gas_frac = 0.0;
                true
            }
            _ => false,
        }
    }

    fn apply_stellar_and_exotic_preset(&mut self, preset: BuilderPreset) {
        match preset {
            BuilderPreset::RedDwarfStar => {
                self.custom_name = "Companion Star".to_string();
                self.mass_solar = 0.15;
                self.semi_major_axis_au = 12.0;
                self.eccentricity = 0.05;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 0.0;
                self.gas_frac = 1.0;
            }
            BuilderPreset::OrangeDwarf => {
                self.custom_name = "Orange Dwarf (K-Type)".to_string();
                self.mass_solar = 0.65;
                self.semi_major_axis_au = 16.0;
                self.eccentricity = 0.03;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 0.0;
                self.gas_frac = 1.0;
            }
            BuilderPreset::WolfRayet => {
                self.custom_name = "Wolf-Rayet Star".to_string();
                self.mass_solar = 28.0;
                self.semi_major_axis_au = 45.0;
                self.eccentricity = 0.10;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 0.05;
                self.gas_frac = 0.95;
            }
            BuilderPreset::PopIIIStar => {
                self.custom_name = "Population III Hypergiant".to_string();
                self.mass_solar = 120.0;
                self.semi_major_axis_au = 60.0;
                self.eccentricity = 0.05;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 0.0;
                self.gas_frac = 1.0;
            }
            BuilderPreset::QuarkStar => {
                self.custom_name = "Strange Quark Star".to_string();
                self.mass_solar = 1.85;
                self.semi_major_axis_au = 25.0;
                self.eccentricity = 0.08;
                self.rock_frac = 0.10;
                self.ice_frac = 0.0;
                self.metal_frac = 0.90;
                self.gas_frac = 0.0;
            }
            BuilderPreset::DarkStar => {
                self.custom_name = "WIMP Dark Star".to_string();
                self.mass_solar = 50.0;
                self.semi_major_axis_au = 50.0;
                self.eccentricity = 0.04;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 0.0;
                self.gas_frac = 1.0;
            }
            BuilderPreset::BosonStar => {
                self.custom_name = "Axion Boson Star".to_string();
                self.mass_solar = 2.5;
                self.semi_major_axis_au = 30.0;
                self.eccentricity = 0.02;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 0.0;
                self.gas_frac = 1.0;
            }
            BuilderPreset::IronStar => {
                self.custom_name = "Cold Iron Star".to_string();
                self.mass_solar = 1.2;
                self.semi_major_axis_au = 20.0;
                self.eccentricity = 0.01;
                self.rock_frac = 0.0;
                self.ice_frac = 0.0;
                self.metal_frac = 1.0;
                self.gas_frac = 0.0;
            }
            BuilderPreset::ThorneZytkow => {
                self.custom_name = "Thorne-Żytkow Object".to_string();
                self.mass_solar = 14.0;
                self.semi_major_axis_au = 40.0;
                self.eccentricity = 0.06;
                self.rock_frac = 0.02;
                self.ice_frac = 0.0;
                self.metal_frac = 0.10;
                self.gas_frac = 0.88;
            }
            _ => {}
        }
    }
}
