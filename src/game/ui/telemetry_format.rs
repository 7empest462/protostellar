//! Formatting helpers for celestial body telemetry readouts and HUD inspector metrics.

use std::fmt::Write;

use crate::simulation::atmosphere_escape::{AtmosphericEscapeRegime, AtmosphericEscapeState};
use crate::simulation::components::*;
use crate::simulation::kozai_lidov::types::KozaiLidovState;
use crate::simulation::relativity::types::RelativisticState;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;
use crate::utils::constants::*;

pub fn format_body_inspector_type(body_type: BodyType, mass_solar: f64) -> &'static str {
    match body_type {
        BodyType::Protostar => "Central Star (Protostar)",
        BodyType::MainSequenceStar => "Main Sequence Star",
        BodyType::BrownDwarf => "Brown Dwarf (Sub-Stellar)",
        BodyType::RedDwarf => "Red Dwarf Star (M-Type)",
        BodyType::YellowDwarf => "Yellow Dwarf Star (G2V)",
        BodyType::BlueGiant => "Blue Giant Star (B-Type)",
        BodyType::BlueSupergiant => "Blue Supergiant Star (O-Type)",
        BodyType::RedGiant => "Red Giant Star",
        BodyType::RedSupergiant => "Red Supergiant Star",
        BodyType::Hypergiant => "Luminous Hypergiant",
        BodyType::WolfRayet => "Wolf-Rayet Star",
        BodyType::WhiteDwarf => "White Dwarf Remnant",
        BodyType::NeutronStar => "Neutron Star Remnant",
        BodyType::Pulsar => "Relativistic Pulsar Remnant",
        BodyType::Magnetar => "Magnetar Remnant",
        BodyType::BlackHole => {
            if mass_solar >= 100_000.0 {
                "Supermassive Black Hole"
            } else if mass_solar >= 100.0 {
                "Intermediate-Mass Black Hole"
            } else {
                "Stellar-Mass Black Hole"
            }
        }
        BodyType::QuasiStar => "Quasi-Star / Black Hole Star (JWST Little Red Dot)",
        BodyType::GasGiant => "Gas Giant Planet",
        BodyType::IceGiant => "Ice Giant Planet",
        BodyType::SuperEarth => "Super-Earth Planet",
        BodyType::TerrestrialPlanet => "Terrestrial Planet",
        BodyType::Protoplanet => "Protoplanetary Embryo",
        BodyType::Planetesimal => "Planetesimal",
        BodyType::Asteroid => "Asteroid",
        BodyType::Comet => "Comet",
        BodyType::DustGrain => "Dust Grain",
        BodyType::DebrisRing => "Debris Ring",
        BodyType::Moon => "Natural Moon / Satellite",
    }
}

pub fn format_stellar_extra_telemetry(
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
    rad: &Radius,
    temp: &Temperature,
    config: &SimulationConfig,
) -> String {
    let Some(ignition) = opt_ignition else {
        return String::new();
    };

    let core_temp_mk = ignition.core_temperature / 1.0e6;
    let fusion_pct = ignition.fusion_fraction * 100.0;
    let evo_str = if let Some(evo) = opt_evo {
        match evo.phase {
            StellarEvolutionPhase::ProtostarContraction => {
                format!(
                    "Hayashi Track Contraction (Fuel: {:.0}% H)",
                    evo.hydrogen_core_fraction * 100.0
                )
            }
            StellarEvolutionPhase::MainSequence => {
                format!(
                    "Stable Main Sequence (Core Fuel: {:.1}% H)",
                    evo.hydrogen_core_fraction * 100.0
                )
            }
            StellarEvolutionPhase::RedGiantBranch => {
                format!(
                    "RED GIANT BRANCH (R: {:.2} AU | L: {:.0} L☉ | Engulfing Inner Planets)",
                    rad.0,
                    (rad.0 / SOLAR_RADIUS_AU).powi(2) * (temp.0 / 5778.0).powi(4)
                )
            }
            StellarEvolutionPhase::RedSupergiantBranch => {
                format!(
                    "RED SUPERGIANT BRANCH (R: {:.2} AU | Massive Core Burning)",
                    rad.0
                )
            }
            StellarEvolutionPhase::SupernovaExplosion => {
                format!(
                    "💥 SUPERNOVA CORE-COLLAPSE (Blast: {:.1} AU @ 15,000 km/s)",
                    evo.nebula_expansion_radius_au
                )
            }
            StellarEvolutionPhase::HeliumFlashAgb => {
                format!(
                    "AGB SUPERGIANT (Core He Fuel: {:.1}% | R: {:.2} AU)",
                    evo.helium_core_fraction * 100.0,
                    rad.0
                )
            }
            StellarEvolutionPhase::PlanetaryNebulaEjection => {
                format!(
                    "PLANETARY NEBULA EJECTION (Shell: {:.1} AU | Shedding Envelope Mass)",
                    evo.nebula_expansion_radius_au
                )
            }
            StellarEvolutionPhase::WhiteDwarf => {
                format!(
                    "DEGENERATE WHITE DWARF REMNANT (Earth-Sized Core | T: {:.0} K | B: 10^6 G)",
                    temp.0
                )
            }
            StellarEvolutionPhase::NeutronStarPulsar => {
                "⚡ NEUTRON STAR / PULSAR REMNANT (B: 10^12 G | Synchrotron Lighthouse Jets)"
                    .to_string()
            }
            StellarEvolutionPhase::MagnetarRemnant => {
                "🧲 MAGNETAR REMNANT (B: 10^15 G | Extreme Magnetic Reconnection Arcs)".to_string()
            }
            StellarEvolutionPhase::BlackHoleRemnant => {
                "🕳️ STELLAR-MASS BLACK HOLE (Event Horizon & Relativistic Accretion Disk)"
                    .to_string()
            }
        }
    } else {
        "Active Hydrogen Fusion".to_string()
    };

    let status = if ignition.is_ignited {
        format!(
            "{}\nSolar Wind Shockwave: {:.2} AU | Gas Dispersal: {:.0}%",
            evo_str,
            ignition.shockwave_radius,
            (1.0 - config.gas_density_scale) * 100.0
        )
    } else {
        format!("Kelvin-Helmholtz Core Heating (Progress: {fusion_pct:.1}%)\nIgnition Threshold: 10.0 MK [Press 'I' or Click Button Below to Ignite]")
    };

    format!("\nStellar Core Temp: {core_temp_mk:.2} MK | Fusion: {fusion_pct:.1}%\nStellar State: {status}")
}

pub fn format_geology_and_volatiles_telemetry(
    opt_diff: Option<&InternalDifferentiation>,
    opt_vol: Option<&VolatileInventory>,
    opt_rings: Option<&PlanetaryRingSystem>,
) -> String {
    let mut out = String::new();

    if let Some(diff) = opt_diff {
        if diff.is_differentiated {
            let core_km = diff.core_radius_au * AU_TO_KM;
            let mantle_km = (diff.mantle_radius_au - diff.core_radius_au).max(0.0) * AU_TO_KM;
            let crust_km = diff.crust_thickness_au * AU_TO_KM;
            let _ = write!(
                out,
                "\nStructure: Differentiated (Core: {:.0} km | Mantle: {:.0} km | Crust: {:.0} km)\nDynamo: {:.2} G | Core Temp: {:.0} K",
                core_km, mantle_km, crust_km, diff.magnetic_field_gauss, diff.core_temp_k
            );
            if diff.has_theia_llsvp {
                let _ = write!(
                    out,
                    "\nMantle Blobs (LLSVPs): +{:.1}% denser (Theia basal mantle remnants)",
                    diff.llsvp_density_contrast * 100.0
                );
            }
        } else {
            out.push_str("\nStructure: Undifferentiated Chondritic Mixture");
        }
    }

    if let Some(vol) = opt_vol {
        let _ = write!(
            out,
            "\nVolatiles: {:.4} M_earth Water Delivered | Ocean Coverage: {:.0}%\nAtmospheric Pressure: {:.2} bar | Icy Bombardment Impacts: {}",
            vol.delivered_water_m_earth,
            vol.ocean_coverage_frac * 100.0,
            vol.atmospheric_pressure_bar,
            vol.cometary_impact_count
        );
    }

    if let Some(ring) = opt_rings {
        let inner_km = f64::from(ring.inner_radius_au) * AU_TO_KM;
        let outer_km = f64::from(ring.outer_radius_au) * AU_TO_KM;
        let _ = write!(
            out,
            "\nRing System: Active (Span: {:.0} - {:.0} km | Opacity: {:.0}% | {:.0}% Ice)",
            inner_km,
            outer_km,
            ring.optical_depth * 100.0,
            ring.ice_fraction * 100.0
        );
    }

    out
}

pub fn format_atmospheric_escape_telemetry(
    opt_escape: Option<&AtmosphericEscapeState>,
    opt_tail: Option<&AtmosphericEscapeTail>,
) -> String {
    let mut out = String::new();
    if let Some(esc) = opt_escape {
        if esc.total_loss_rate_m_earth_per_myr > 0.0001
            || esc.escape_regime != AtmosphericEscapeRegime::None
        {
            let tail_str = opt_tail.map_or(String::new(), |t| {
                if t.is_active && t.tail_length_au > 0.01 {
                    format!(" | Tail: {:.2} AU", t.tail_length_au)
                } else {
                    String::new()
                }
            });
            let _ = write!(
                out,
                "\nAtmosphere: {} ({:.2} M_earth/Myr{}) | Shielding: {:.0}%",
                esc.escape_regime.label(),
                esc.total_loss_rate_m_earth_per_myr,
                tail_str,
                esc.magnetic_shielding_factor * 100.0,
            );
        }
    } else if let Some(tail) = opt_tail {
        if tail.is_active && tail.tail_length_au > 0.01 {
            let _ = write!(
                out,
                "\nPhotoevaporation: Active (Loss: {:.2} M_earth/Myr | Tail: {:.2} AU)",
                tail.loss_rate_m_earth_per_myr, tail.tail_length_au
            );
        }
    }
    out
}

pub fn format_climate_biosphere_telemetry(
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_tide: Option<&TidalState>,
) -> String {
    let mut out = String::new();

    if let Some(climate) = opt_climate {
        let regime_name = match climate.climate_regime {
            ClimateRegime::SnowballIceAge => "Frozen Snowball (Ice Age)",
            ClimateRegime::TemperateHabitable => "Temperate Habitable",
            ClimateRegime::RunawayVenusian => "Runaway Greenhouse (Venusian)",
            ClimateRegime::GasGiantEnvelope => "Gas Giant Envelope",
            ClimateRegime::AirlessVacuum => "Airless Vacuum",
        };
        let ice_cap_str = if climate.polar_ice_cap_latitude_deg < 89.0 {
            format!(" | Ice Cap: {:.0}° lat", climate.polar_ice_cap_latitude_deg)
        } else {
            String::new()
        };
        let _ = write!(
            out,
            "\nClimate: {} (T: {:.0} K | Albedo: {:.2}{})",
            regime_name, climate.surface_temperature_k, climate.albedo, ice_cap_str
        );
    }

    if let Some(bio) = opt_bio {
        let status = if bio.biomass_coverage_frac >= 0.50 {
            "Thriving Eden"
        } else if bio.biomass_coverage_frac >= 0.05 {
            "Colonizing Biosphere"
        } else if bio.habitability_score >= 0.40 {
            "Pre-Biotic Prime"
        } else {
            "Sterile / Hostile"
        };
        let _ = write!(
            out,
            "\nBiosphere: {} (Biomass: {:.0}% | O2: {:.1}% | Habitability: {:.0}%)",
            status,
            bio.biomass_coverage_frac * 100.0,
            bio.oxygen_fraction * 100.0,
            bio.habitability_score * 100.0
        );
    }

    if let Some(tide) = opt_tide {
        let lock_label = if tide.is_tidally_locked {
            if (tide.resonance_ratio - 1.0).abs() < 0.05 {
                "Synchronous (1:1)".to_string()
            } else {
                format!("Resonant ({:.1}:1)", tide.resonance_ratio)
            }
        } else {
            format!("Locking ({:.0}%)", tide.locking_progress * 100.0)
        };
        let flux_mw = tide.tidal_heating_flux_w_m2 * 1000.0;
        let _ = write!(
            out,
            "\nTidal State: {} (Flux: {:.1} mW/m² | de/dt: {:.2e}/Myr)",
            lock_label, flux_mw, tide.circularization_rate_per_myr
        );
    }

    out
}

pub fn format_relativistic_telemetry(opt_rel: Option<&RelativisticState>) -> String {
    let mut out = String::new();

    if let Some(rel) = opt_rel {
        if rel.precession_rate_arcsec_century > 0.01 || rel.gw_luminosity_watts > 1.0 {
            let advance_deg = rel.precession_advance_per_orbit_rad.to_degrees();
            let prec_str = if advance_deg >= 0.01 {
                format!("+{advance_deg:.2}°/orb")
            } else {
                format!("+{:.1}″/cy", rel.precession_rate_arcsec_century)
            };
            let status = if rel.is_coalescing {
                "COALESCING"
            } else {
                "Stable"
            };
            let _ = write!(
                out,
                "\n1PN Precession: {} (+{:.3}° total) | Status: {}",
                prec_str,
                rel.accumulated_precession_rad.to_degrees(),
                status
            );
            if rel.total_redshift_z > 1e-7 || rel.beta_v_over_c > 0.001 {
                let _ = write!(
                    out,
                    "\nRelativity: z = {:.4e} | β = {:.2}% c",
                    rel.total_redshift_z,
                    rel.beta_v_over_c * 100.0
                );
            }
            if rel.gw_luminosity_watts > 10.0 {
                let _ = write!(
                    out,
                    "\nGravitational Waves: {:.2e} W (f_GW: {:.2e} Hz | h: {:.1e}) | Inspiral: {:.2e} yr",
                    rel.gw_luminosity_watts,
                    rel.gw_frequency_hz,
                    rel.gw_strain,
                    rel.inspiral_timescale_yr
                );
            }
        }
    }

    out
}

#[allow(
    clippy::too_many_arguments,
    reason = "Formatted telemetry display incorporates all astrophysical state components into a cohesive readout"
)]
pub fn format_body_environment_telemetry(
    opt_diff: Option<&InternalDifferentiation>,
    opt_vol: Option<&VolatileInventory>,
    opt_rings: Option<&PlanetaryRingSystem>,
    opt_tail: Option<&AtmosphericEscapeTail>,
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
    opt_tide: Option<&TidalState>,
    opt_rel: Option<&RelativisticState>,
    opt_escape: Option<&AtmosphericEscapeState>,
    opt_kozai: Option<&KozaiLidovState>,
    rad: &Radius,
    temp: &Temperature,
    config: &SimulationConfig,
) -> String {
    let mut out = String::new();
    out.push_str(&format_geology_and_volatiles_telemetry(
        opt_diff, opt_vol, opt_rings,
    ));
    out.push_str(&format_climate_biosphere_telemetry(
        opt_climate,
        opt_bio,
        opt_tide,
    ));
    out.push_str(&format_relativistic_telemetry(opt_rel));
    out.push_str(&format_atmospheric_escape_telemetry(opt_escape, opt_tail));
    out.push_str(&crate::game::ui::inspector_panel::format_kozai_lidov_telemetry(opt_kozai));
    out.push_str(&format_stellar_extra_telemetry(
        opt_ignition,
        opt_evo,
        rad,
        temp,
        config,
    ));
    out
}

pub fn format_composition_line(
    comp: &Composition,
    opt_vol: Option<&VolatileInventory>,
    opt_climate: Option<&PlanetaryClimate>,
    temp_k: f64,
    is_star: bool,
) -> String {
    let norm = comp.normalized();
    let rock_pct = ((norm.silicate_frac + norm.organics_frac) * 100.0).round();
    let ice_pct = (norm.ice_frac * 100.0).round();
    let metal_pct = (norm.metal_frac * 100.0).round();
    let gas_pct = (100.0f64 - rock_pct - ice_pct - metal_pct).max(0.0);
    let water_ice_str = crate::game::ui::inspector_panel::format_composition_water_ice(
        comp,
        opt_vol,
        opt_climate,
        temp_k,
        is_star,
    );
    format!("{rock_pct:.0}% Rock | {water_ice_str} | {metal_pct:.0}% Metal | {gas_pct:.0}% Gas")
}

pub fn format_mass_string(mass_val: f64) -> String {
    let m_earth = mass_val / EARTH_MASS_SOLAR;
    if mass_val >= 10_000.0 {
        format!("{mass_val:.0} M☉ (Supermassive Seed)")
    } else if mass_val >= 0.01 {
        format!(
            "{mass_val:.3} M_sun ({:.1} M_J)",
            mass_val / JUPITER_MASS_SOLAR
        )
    } else if m_earth >= 0.01 {
        format!("{m_earth:.2} M_earth ({mass_val:.4} M_sun)")
    } else if m_earth >= 1e-4 {
        format!("{m_earth:.4} M_earth ({mass_val:.2e} M_sun)")
    } else {
        format!("{m_earth:.2e} M_earth ({mass_val:.2e} M_sun)")
    }
}

pub fn format_orbital_period_string(dist_au: f64, body_type: BodyType, star_mass: f64) -> String {
    if dist_au > 0.05 && !body_type.is_star_or_remnant() {
        let p_yr = dist_au.powf(1.5) / star_mass.max(0.1).sqrt();
        if p_yr >= 1.0 {
            format!(" | Period: {p_yr:.2} yr")
        } else {
            format!(" | Period: {:.1} days", p_yr * 365.25)
        }
    } else {
        String::new()
    }
}

pub fn append_quasi_star_telemetry(out: &mut String, qs: &BlackHoleStarState) {
    let acc_mode = if qs.super_eddington_active {
        "SUPER-EDDINGTON (4.5x)"
    } else {
        "SUB-EDDINGTON (0.9x)"
    };
    let status = if qs.is_blown_out {
        format!(
            "QUASAR TRANSITION (Progress: {:.0}%)",
            qs.blowout_progress * 100.0
        )
    } else {
        format!("HYDROGEN COCOON INTACT ({:.0} AU)", qs.cocoon_radius_au)
    };
    let _ = write!(
        out,
        "\n--------------------------------------------------\n  >> JWST LITTLE RED DOT / QUASI-STAR <<\n--------------------------------------------------\n  • BH Seed Mass:     {:>10.0} M☉\n  • Cocoon Mass:      {:>10.0} M☉\n  • Inflow Rate:      {:>10.1}x ({})\n  • Cocoon Status:    {}\n  • Redshift Epoch:   z ≈ 8.5 (Cosmic Dawn, 660 Myr)\n  • Controls:         [X] Accrete | [B] Blowout | [T] Pop-III TDE\n--------------------------------------------------",
        qs.black_hole_mass_solar,
        qs.cocoon_mass_solar,
        qs.eddington_ratio,
        acc_mode,
        status,
    );
}
