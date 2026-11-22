//! Pure telemetry text formatters for the Target Inspector panel.

use crate::simulation::components::*;

/// Formats the volatile H2O mass component for the bulk composition readout (percentage of total mass),
/// distinguishing liquid water from frozen ice based on temperature.
pub fn format_composition_water_ice(
    comp: &Composition,
    opt_vol: Option<&VolatileInventory>,
    opt_climate: Option<&PlanetaryClimate>,
    temp_k: f64,
    is_star: bool,
) -> String {
    let norm = comp.normalized();
    let ice_pct = (norm.ice_frac * 100.0).round();
    if is_star {
        return format!("{ice_pct:.0}% Ice");
    }

    let surf_temp_k = opt_climate.map_or(temp_k, |c| f64::from(c.surface_temperature_k));
    let is_warm = temp_k >= 273.0 || surf_temp_k >= 273.0;
    let has_water =
        norm.ice_frac > 0.0001 || opt_vol.is_some_and(|v| v.delivered_water_m_earth > 1e-6);

    if is_warm {
        if ice_pct >= 1.0 {
            format!("{ice_pct:.0}% Water")
        } else if has_water {
            "<1% Water".to_string()
        } else {
            "0% Water".to_string()
        }
    } else if ice_pct >= 1.0 {
        format!("{ice_pct:.0}% Ice")
    } else if has_water {
        "<1% Ice".to_string()
    } else {
        "0% Ice".to_string()
    }
}

/// Formats Kozai-Lidov secular resonance telemetry for the Target Inspector panel.
pub fn format_kozai_lidov_telemetry(
    opt_kozai: Option<&crate::simulation::kozai_lidov::KozaiLidovState>,
) -> String {
    let mut out = String::new();
    if let Some(kozai) = opt_kozai
        && (kozai.is_in_resonance || kozai.mutual_inclination_deg > 5.0)
    {
        use std::fmt::Write;
        let status = if kozai.is_in_resonance {
            format!("ACTIVE ({})", kozai.regime.label())
        } else {
            "Subcritical".to_string()
        };
        let _ = write!(
            out,
            "\nKozai-Lidov: {} | Mut Inc: {:.1}° (Crit: {:.1}°)",
            status, kozai.mutual_inclination_deg, kozai.critical_inclination_deg
        );
        if kozai.is_in_resonance {
            let _ = write!(
                out,
                "\ne_max: {:.2} (q_min: {:.3} AU) | τ_KL: {:.1e} yr",
                kozai.max_eccentricity_forecast, kozai.min_periastron_au, kozai.kozai_period_years
            );
            if kozai.is_gr_suppressed {
                let _ = write!(
                    out,
                    " | [GR Suppressed: τ_KL/τ_GR = {:.1}]",
                    kozai.gr_precession_ratio
                );
            }
            if kozai.regime == crate::simulation::kozai_lidov::KozaiRegime::TidalDisruptionRisk {
                out.push_str(" ⚠️ ROCHE DANGER");
            }
        }
    }
    out
}

/// Formats relativistic polar jet and synchrotron emission cone telemetry for the Target Inspector panel.
pub fn append_relativistic_jet_telemetry(out: &mut String, jet: &RelativisticJetState) {
    use std::fmt::Write;
    let beta = crate::simulation::relativity::calculate_relativistic_velocity(jet.lorentz_factor);
    let beam_deg = crate::simulation::relativity::calculate_beaming_half_angle(jet.lorentz_factor)
        .to_degrees();
    let cone_deg = jet.opening_angle_rad.to_degrees();
    let _ = write!(
        out,
        "\n━━━ ⟪ RELATIVISTIC JET & SYNCHROTRON CONE ⟫ ━━━━━━━━\n  • Lorentz Factor (Γ): {:>8.1} (v = {:.4}c)\n  • Beaming Half-Angle: {:>8.1}°\n  • Collimation Cone:   {:>8.1}°\n  • Jet Extent:         {:>8.1} AU\n  • Shock Knots:        {:>8.1}x ({:.2}c)\n  • Synchrotron Lum:    {:>8.1} L☉ (p={:.2})\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
        jet.lorentz_factor,
        beta,
        beam_deg,
        cone_deg,
        jet.jet_length_au,
        jet.knot_frequency,
        jet.knot_speed_c,
        jet.synchrotron_luminosity,
        jet.spectral_index,
    );
}

/// Formats space weather telemetry (stellar CME flares and planetary auroral ovals) for the Target Inspector panel.
pub fn append_space_weather_telemetry(
    out: &mut String,
    opt_aurora: Option<&crate::simulation::space_weather::AuroralOvalState>,
    opt_flare: Option<&crate::simulation::space_weather::StellarFlareState>,
    is_gas_giant: bool,
) {
    use std::fmt::Write;
    if let Some(flare) = opt_flare {
        let flare_status = if flare.current_flare_intensity > 0.1 {
            let class_str = if flare.current_flare_intensity >= 5.0 {
                format!("SUPERFLARE ({:.1}x)", flare.current_flare_intensity)
            } else if flare.current_flare_intensity >= 2.0 {
                format!("X-CLASS ({:.1})", flare.current_flare_intensity * 2.0)
            } else {
                format!("M-CLASS ({:.1})", flare.current_flare_intensity * 5.0)
            };
            format!(
                "ACTIVE [{class_str}] (Decay: {:.2} yr)",
                flare.flare_decay_timer_years
            )
        } else {
            "Quiescent / Solar Minimum".to_string()
        };

        let cme_str = if flare.cme_active {
            let speed_km_s =
                flare.cme_speed_au_day * (crate::utils::constants::AU_TO_KM as f32) / 86400.0;
            format!(
                "PROPAGATING | Front: {:.2} AU | Speed: {:.0} km/s ({:.2} AU/day) | Shock: {:.0}x",
                flare.cme_front_radius_au,
                speed_km_s,
                flare.cme_speed_au_day,
                flare.cme_density_multiplier
            )
        } else {
            "No active CME shock front".to_string()
        };

        let _ = write!(
            out,
            "\n━━━ ⟪ STELLAR CORONA & SPACE WEATHER ⟫ ━━━━━━━━━━━━\n  • Coronal Activity: {}\n  • CME Shockwave:    {}\n  • Flare Frequency:  {:.1} / year\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            flare_status, cme_str, flare.flare_frequency
        );
    }

    if let Some(aurora) = opt_aurora {
        let lat_deg = 90.0 - aurora.oval_colatitude_rad.to_degrees();
        let width_deg = aurora.oval_width_rad.to_degrees();
        let emission_str = if is_gas_giant {
            "H-α 656.3nm / H₃⁺ IR (FUV)"
        } else {
            "O 557.7nm / N₂⁺ 391.4nm"
        };
        let _ = write!(
            out,
            "\n━━━ ⟪ MAGNETOSPHERE & AURORAL OVAL ⟫ ━━━━━━━━━━━━━━\n  • Space Weather:     {} (Kp {:1.1})\n  • Magnetopause:      {:.1} Rp (Subsolar Standoff)\n  • Auroral Footprint: ±{:.1}° Lat (θ_A = {:.1}°) | Width: {:.1}°\n  • Auroral Emission:  {:.0}% [{emission_str}]\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            aurora.storm_level.label(),
            aurora.geomagnetic_kp_index,
            aurora.magnetopause_standoff_rp,
            lat_deg,
            aurora.oval_colatitude_rad.to_degrees(),
            width_deg,
            aurora.auroral_intensity * 100.0
        );
    }
}

/// Formats circumstellar accretion disk telemetry (age, mass budget, active full disk state) for the Target Inspector panel.
pub fn append_circumstellar_disk_telemetry(out: &mut String, disk: &CircumstellarDisk) {
    use std::fmt::Write;
    let status_str = if disk.is_active_full_disk {
        "ACTIVE (Full 3D Accretion)"
    } else {
        "CLEARED / DISSIPATED"
    };
    let age_str = if disk.age_years >= 1_000_000.0 {
        format!("{:.2} Myr", disk.age_years / 1_000_000.0)
    } else if disk.age_years >= 1_000.0 {
        format!("{:.1} kyr", disk.age_years / 1_000.0)
    } else {
        format!("{:.0} yr", disk.age_years)
    };
    let lifetime_str = if disk.lifetime_years >= 1_000_000.0 {
        format!("{:.2} Myr", disk.lifetime_years / 1_000_000.0)
    } else {
        format!("{:.0} kyr", disk.lifetime_years / 1_000.0)
    };
    let mass_str = if disk.disk_mass_solar >= 0.001 {
        format!(
            "{:.4} M☉ ({:.1} M_J)",
            disk.disk_mass_solar,
            disk.disk_mass_solar / crate::utils::constants::JUPITER_MASS_SOLAR
        )
    } else {
        format!(
            "{:.2} M_earth",
            disk.disk_mass_solar / crate::utils::constants::EARTH_MASS_SOLAR
        )
    };
    let _ = write!(
        out,
        "\n━━━ ⟪ CIRCUMSTELLAR ACCRETION DISK ⟫ ━━━━━━━━━━━━━━\n  • Status:     {status_str}\n  • Disk Age:   {age_str} / {lifetime_str}\n  • Disk Mass:  {mass_str}\n  • Extent:     {:.1} - {:.1} AU\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
        disk.inner_radius_au, disk.outer_radius_au
    );
}
