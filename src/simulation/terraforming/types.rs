//! Types, components, and events for targeted terraforming and orbital bombardment.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Type of targeted orbital bombardment projectile or salvo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BombardmentType {
    /// Water-rich cometary nucleus (60% Ice, 20% Organics, 20% Silicate). Hydrosphere & ocean builder.
    #[default]
    IcyComet,
    /// Carbonaceous chondrite / volatile asteroid (35% Organics, 25% Ice, 40% Silicate). Builds N2/CO2 atmosphere.
    CarbonaceousChondrite,
    /// Aerobraking volatile swarm designed for gentle atmospheric ablation without heavy cratering.
    VolatileAblationSalvo,
    /// Dense iron-nickel impactor (75% Metal, 25% Silicate). Deep impact for mantle heating & geodynamo stimulation.
    IronAsteroid,
}

impl BombardmentType {
    pub fn name(&self) -> &'static str {
        match self {
            BombardmentType::IcyComet => "Icy Comet (H₂O)",
            BombardmentType::CarbonaceousChondrite => "Carbonaceous Chondrite (Atmosphere)",
            BombardmentType::VolatileAblationSalvo => "Aerobraking Volatile Salvo",
            BombardmentType::IronAsteroid => "Iron Core Impactor",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            BombardmentType::IcyComet => Color::srgb(0.35, 0.85, 1.0),
            BombardmentType::CarbonaceousChondrite => Color::srgb(1.0, 0.65, 0.25),
            BombardmentType::VolatileAblationSalvo => Color::srgb(0.75, 0.45, 1.0),
            BombardmentType::IronAsteroid => Color::srgb(1.0, 0.35, 0.25),
        }
    }
}

/// Component attached to an in-flight guided bombardment projectile.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct BombardmentProjectile {
    /// Entity of the target planetary world
    pub target_entity: Entity,
    /// Classification of this bombardment projectile
    pub bombardment_type: BombardmentType,
    /// Simulation timestamp when launched (yr)
    pub launch_time_yr: f64,
    /// Estimated target arrival timestamp (yr)
    pub expected_arrival_yr: f64,
    /// Trajectory color tint for rendering
    pub trail_color: Color,
    /// Whether this projectile has already initiated impact resolution
    pub is_detonated: bool,
}

/// Dynamic multi-species atmospheric composition and terraforming progress.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TerraformingAtmosphere {
    /// Carbon dioxide partial pressure in bars
    pub co2_pressure_bar: f32,
    /// Nitrogen (N2) buffer gas partial pressure in bars
    pub nitrogen_pressure_bar: f32,
    /// Atmospheric water vapor currently suspended in steam envelope (M_earth)
    pub atmospheric_water_m_earth: f64,
    /// Liquid surface water condensed into basins and oceans (M_earth)
    pub surface_liquid_water_m_earth: f64,
    /// Suspended impact dust optical depth causing transient impact winter cooling
    pub impact_dust_optical_depth: f32,
    /// Cumulative mass of volatiles and impactors delivered to this world (M_earth)
    pub total_bombarded_mass_earth: f64,
}

impl Default for TerraformingAtmosphere {
    fn default() -> Self {
        Self {
            co2_pressure_bar: 0.0004,
            nitrogen_pressure_bar: 0.78,
            atmospheric_water_m_earth: 0.00005,
            surface_liquid_water_m_earth: 0.002,
            impact_dust_optical_depth: 0.0,
            total_bombarded_mass_earth: 0.0,
        }
    }
}

impl TerraformingAtmosphere {
    /// Returns the total atmospheric surface pressure in bars.
    pub fn total_pressure_bar(&self) -> f32 {
        let vapor_pressure_bar = (self.atmospheric_water_m_earth * 250.0) as f32;
        (self.co2_pressure_bar + self.nitrogen_pressure_bar + vapor_pressure_bar).max(0.0)
    }
}

/// Event broadcast when a guided bombardment body impacts a target world.
#[derive(Event, Message, Debug, Clone)]
pub struct BombardmentEvent {
    pub target_entity: Entity,
    pub target_name: String,
    pub bombardment_type: BombardmentType,
    pub water_delivered_m_earth: f64,
    pub gas_added_bar: f32,
    pub impact_velocity_km_s: f64,
}

/// Calculated outcome of an atmospheric entry and surface strike.
#[derive(Debug, Clone, Copy)]
pub struct ImpactDeliveryResult {
    /// Liquid water delivered directly to surface basins (M_earth)
    pub liquid_water_delivered_m_earth: f64,
    /// Vaporized steam added to the atmosphere (M_earth)
    pub steam_vapor_delivered_m_earth: f64,
    /// New CO2 pressure added to atmosphere (bar)
    pub delta_co2_bar: f32,
    /// New N2 pressure added to atmosphere (bar)
    pub delta_nitrogen_bar: f32,
    /// Transient impact dust optical depth generated
    pub delta_dust_opacity: f32,
    /// Core/interior thermal boost in Kelvin
    pub core_temp_boost_k: f64,
    /// Angular radius of the resulting impact crater on the sphere
    pub crater_angular_radius: f32,
    /// Entry impact velocity in km/s
    pub impact_speed_km_s: f64,
}
