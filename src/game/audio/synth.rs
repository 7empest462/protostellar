//! Procedural 16-bit PCM RIFF/WAVE synthesis for cosmic events, ambience loops, and UI cues.

#[inline]
pub(crate) fn quantize_pcm_sample(v: f32) -> i16 {
    (v * f32::from(i16::MAX)).clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

#[inline]
fn next_white_noise(rng_state: &mut u32) -> f32 {
    *rng_state = rng_state
        .wrapping_mul(1_664_525)
        .wrapping_add(1_013_904_223);
    ((*rng_state >> 9) as f32 / ((1u32 << 23) as f32)) * 2.0 - 1.0
}

fn alloc_pcm_wav_with_header(sample_rate: u32, channels: u16, frame_count: usize) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = bits_per_sample / 8;
    let block_align: u16 = channels.saturating_mul(bytes_per_sample);
    let byte_rate: u32 = sample_rate.saturating_mul(u32::from(block_align));
    let data_bytes: u32 = frame_count
        .saturating_mul(usize::from(block_align))
        .min(u32::MAX as usize) as u32;
    let riff_size: u32 = 36_u32.saturating_add(data_bytes);

    let mut out = Vec::with_capacity(44 + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    out
}

/// Encodes mono 16-bit signed PCM samples into a complete 44-byte header RIFF/WAVE buffer.
pub fn synthesize_pcm_wav_mono(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let mut out = alloc_pcm_wav_with_header(sample_rate, 1, samples.len());
    for &sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Encodes interleaved stereo 16-bit signed PCM `(left, right)` frames into a RIFF/WAVE buffer.
pub fn synthesize_pcm_wav_stereo(sample_rate: u32, samples: &[(i16, i16)]) -> Vec<u8> {
    let mut out = alloc_pcm_wav_with_header(sample_rate, 2, samples.len());
    for &(left, right) in samples {
        out.extend_from_slice(&left.to_le_bytes());
        out.extend_from_slice(&right.to_le_bytes());
    }
    out
}

/// Crossfades tail overrun back onto the loop head so seamless loops avoid boundary clicks.
fn crossfade_seamless_stereo_loop(
    raw: &[(f32, f32)],
    total_samples: usize,
    overlap_len: usize,
    gain: f32,
) -> Vec<(i16, i16)> {
    let overlap = overlap_len.min(total_samples / 2).max(1);
    let mut out = Vec::with_capacity(total_samples);
    let half_pi = std::f32::consts::FRAC_PI_2;
    for (i, &head) in raw.iter().take(total_samples).enumerate() {
        let (l, r) = if i < overlap
            && let Some(&tail) = raw.get(total_samples + i)
        {
            let phase = (i as f32 / overlap as f32) * half_pi;
            let fade_in = phase.sin();
            let fade_out = phase.cos();
            (
                head.0 * fade_in + tail.0 * fade_out,
                head.1 * fade_in + tail.1 * fade_out,
            )
        } else {
            head
        };
        out.push((
            quantize_pcm_sample((l * gain).clamp(-1.0, 1.0)),
            quantize_pcm_sample((r * gain).clamp(-1.0, 1.0)),
        ));
    }
    out
}

/// Synthesizes a seamless 2-second stereo deep-space harmonic continuum loop (integer-cycle 55 Hz binaural detune).
pub fn build_ambient_drone_wav(sample_rate: u32) -> Vec<u8> {
    let duration_secs = 2.0_f32;
    let total_samples = ((sample_rate as f32) * duration_secs) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let tau = std::f32::consts::TAU;
    for i in 0..total_samples {
        let t = (i as f32) / (sample_rate as f32);
        let lfo_l = 0.72 + 0.28 * (tau * 0.5 * t).sin();
        let lfo_r = 0.72 + 0.28 * (tau * 0.5 * t + std::f32::consts::FRAC_PI_2).sin();
        let wave_l = 0.55 * (tau * 55.0 * t).sin()
            + 0.28 * (tau * 110.0 * t).sin()
            + 0.14 * (tau * 165.0 * t).sin();
        let wave_r = 0.55 * (tau * 55.5 * t).sin()
            + 0.28 * (tau * 110.0 * t).sin()
            + 0.14 * (tau * 165.0 * t).cos();
        samples.push((
            quantize_pcm_sample(wave_l * lfo_l * 0.42),
            quantize_pcm_sample(wave_r * lfo_r * 0.42),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a stereo LIGO-style accelerating gravitational-wave chirp and ringdown chime.
pub fn build_gw_chirp_wav(sample_rate: u32) -> Vec<u8> {
    let duration_secs = 0.9_f32;
    let total_samples = ((sample_rate as f32) * duration_secs) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let tau = std::f32::consts::TAU;
    let mut phase = 0.0_f32;
    let dt = 1.0 / (sample_rate as f32);
    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let (freq, env) = if u < 0.78 {
            let p = u / 0.78;
            (68.0 + 360.0 * p.powi(2), 0.20 + 0.80 * p.powf(1.5))
        } else {
            let tail = (u - 0.78) / 0.22;
            (410.0 - 40.0 * tail, (-tail * 6.0).exp())
        };
        phase += tau * freq * dt;
        let fade_in = (u * 20.0).min(1.0);
        let wave_l = (phase.sin() + 0.3 * (phase * 2.0).sin()) * env * fade_in;
        let wave_r = ((phase + 0.45).sin() + 0.3 * ((phase + 0.45) * 2.0).sin()) * env * fade_in;
        samples.push((
            quantize_pcm_sample(wave_l * 0.45),
            quantize_pcm_sample(wave_r * 0.45),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a stereo dispersed radio-pulsar & starquake acoustic lighthouse pulse burst.
pub fn build_pulsar_pulse_wav(sample_rate: u32) -> Vec<u8> {
    let duration_secs = 0.35_f32;
    let total_samples = ((sample_rate as f32) * duration_secs) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let tau = std::f32::consts::TAU;
    for i in 0..total_samples {
        let t = (i as f32) / (sample_rate as f32);
        let u = (i as f32) / (total_samples.max(1) as f32);
        let env = (u * 30.0).min(1.0) * (-u * 8.0).exp();
        let sweep = 240.0 + 180.0 * (1.0 - u);
        let pulse = (tau * sweep * t).sin() * 0.7 + (tau * 18.0 * t).sin() * 0.3;
        let angle = (0.25 + 0.5 * u) * std::f32::consts::FRAC_PI_2;
        samples.push((
            quantize_pcm_sample(pulse * env * 0.45 * angle.cos()),
            quantize_pcm_sample(pulse * env * 0.45 * angle.sin()),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a crisp, subtle 28 ms dual-tone sci-fi UI telemetry chirp (1,320 Hz -> 1,980 Hz).
pub fn build_ui_click_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = ((sample_rate as f32) * 0.028_f32).round().max(1.0) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let dt = 1.0 / (sample_rate as f32);
    let mut phase = 0.0_f32;
    for i in 0..total_samples {
        let u = (i as f32) / (total_samples as f32);
        let env = (u * 12.0).min(1.0) * (-u * 8.5).exp();
        let freq = 1320.0 + (1980.0 - 1320.0) * u;
        phase += std::f32::consts::TAU * freq * dt;
        let sample =
            quantize_pcm_sample((phase.sin() * 0.72 + (phase * 1.5).sin() * 0.28) * env * 0.40);
        samples.push((sample, sample));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a 2.4-second stereo seismic sub-bass & broadband plasma detonation boom
/// for the Little Red Dot Quasar Blowout.
pub fn build_quasar_blowout_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let duration_secs = 2.4_f32;
    let total_samples = ((sample_rate as f32) * duration_secs) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let dt = 1.0 / (sample_rate as f32);
    let tau = std::f32::consts::TAU;
    let mut sub_phase = 0.0_f32;
    let mut crack_phase = 0.0_f32;
    let mut rng_state: u32 = 0x51A7_E91D;
    let mut lp_noise = 0.0_f32;

    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let white = next_white_noise(&mut rng_state);
        let alpha = (0.04 + 0.14 * (-u * 3.2).exp()).clamp(0.02, 0.25);
        lp_noise += (white - lp_noise) * alpha;

        let sub_freq = 22.0 + 50.0 * (-u * 4.5).exp();
        sub_phase += tau * sub_freq * dt;
        let crack_freq = 190.0 + 420.0 * (-u * 7.5).exp();
        crack_phase += tau * crack_freq * dt;

        let attack = (u * 35.0).min(1.0);
        let boom_env = attack * (-u * 2.3).exp();
        let shock_punch = (-(u - 0.18).powi(2) / 0.004).exp() * 0.65;
        let sub_wave = (sub_phase.sin() * 0.72 + (sub_phase * 0.5).sin() * 0.28)
            * (boom_env + shock_punch * 0.5);
        let crackle = crack_phase.sin() * (-u * 8.0).exp() * 0.28;
        let roar = lp_noise * (boom_env * 0.85 + shock_punch * 0.55);

        let left = (sub_wave * 0.62 + roar * 0.52 + crackle).clamp(-1.0, 1.0) * 0.72;
        let right = (sub_wave * 0.62 + (roar * 0.92 + white * 0.06 * boom_env) * 0.52
            - crackle * 0.8)
            .clamp(-1.0, 1.0)
            * 0.72;
        samples.push((quantize_pcm_sample(left), quantize_pcm_sample(right)));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a 1.6-second giant planetary impact boom with silicate/metallic debris shimmer.
pub fn build_giant_impact_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = ((sample_rate as f32) * 1.6_f32) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let dt = 1.0 / (sample_rate as f32);
    let tau = std::f32::consts::TAU;
    let mut sub_phase = 0.0_f32;
    let mut rng_state: u32 = 0x6A1B_4C2D;
    let mut lp_noise = 0.0_f32;

    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let t = (i as f32) * dt;
        let white = next_white_noise(&mut rng_state);
        lp_noise += (white - lp_noise) * (0.03 + 0.16 * (-u * 4.5).exp());

        sub_phase += tau * (26.0 + 59.0 * (-u * 5.2).exp()) * dt;
        let attack = (u * 55.0).min(1.0);
        let thump =
            (sub_phase.sin() * 0.72 + (sub_phase * 0.5).sin() * 0.28) * attack * (-u * 3.1).exp();
        let crack = white * (-u * 26.0).exp() * 0.42;
        let scatter = lp_noise * (-u * 2.5).exp() * 0.48;

        let shimmer_env = ((u - 0.08).max(0.0) * 10.0).min(1.0) * (-u * 2.1).exp();
        let chime_l = ((tau * 740.0 * t).sin() * 0.55
            + (tau * 1180.0 * t).sin() * 0.30
            + (tau * 1690.0 * t).sin() * 0.15)
            * shimmer_env
            * 0.22;
        let chime_r = ((tau * 765.0 * t).sin() * 0.55
            + (tau * 1215.0 * t).sin() * 0.30
            + (tau * 1740.0 * t).sin() * 0.15)
            * shimmer_env
            * 0.22;

        let left = (thump * 0.65 + scatter + crack + chime_l).clamp(-1.0, 1.0) * 0.72;
        let right =
            (thump * 0.65 + scatter * 0.92 - crack * 0.65 + chime_r).clamp(-1.0, 1.0) * 0.72;
        samples.push((quantize_pcm_sample(left), quantize_pcm_sample(right)));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a 2.6-second supernova core-collapse implosion rising into a wide stereo shock boom.
pub fn build_supernova_detonation_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = ((sample_rate as f32) * 2.6_f32) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let dt = 1.0 / (sample_rate as f32);
    let tau = std::f32::consts::TAU;
    let split = 0.054_f32;
    let mut core_phase = 0.0_f32;
    let mut rng_state: u32 = 0xD370_9A51;
    let mut lp_l = 0.0_f32;
    let mut lp_r = 0.0_f32;

    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let white_l = next_white_noise(&mut rng_state);
        let white_r = next_white_noise(&mut rng_state);
        if u < split {
            let p = u / split;
            core_phase += tau * (90.0 + 170.0 * p.powi(2)) * dt;
            let s = core_phase.sin() * (0.12 + 0.45 * p.powi(2)) * 0.65;
            samples.push((quantize_pcm_sample(s), quantize_pcm_sample(s * 0.96)));
            continue;
        }
        let v = (u - split) / (1.0 - split);
        let alpha = (0.03 + 0.18 * (-v * 3.0).exp()).clamp(0.02, 0.25);
        lp_l += (white_l - lp_l) * alpha;
        lp_r += (white_r - lp_r) * alpha;
        core_phase += tau * (18.0 + 47.0 * (-v * 4.2).exp()) * dt;
        let shock_env = (v * 45.0).min(1.0) * (-v * 2.2).exp();
        let rumble = (core_phase.sin() * 0.72 + (core_phase * 0.5).sin() * 0.28) * shock_env * 0.72;
        let plasma_l = lp_l * shock_env * 0.62 + white_l * (-v * 14.0).exp() * 0.25;
        let plasma_r = lp_r * shock_env * 0.62 + white_r * (-v * 14.0).exp() * 0.25;
        samples.push((
            quantize_pcm_sample((rumble + plasma_l).clamp(-1.0, 1.0) * 0.74),
            quantize_pcm_sample((rumble + plasma_r).clamp(-1.0, 1.0) * 0.74),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a 2.2-second neutron-star kilonova chirp, GRB snap, and r-process metallic ringdown.
pub fn build_kilonova_detonation_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = ((sample_rate as f32) * 2.2_f32) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let dt = 1.0 / (sample_rate as f32);
    let tau = std::f32::consts::TAU;
    let merge_u = 0.173_f32;
    let mut chirp_phase = 0.0_f32;
    let mut rng_state: u32 = 0xF10B_2A8E;
    let mut lp_noise = 0.0_f32;

    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let white = next_white_noise(&mut rng_state);
        if u < merge_u {
            let p = u / merge_u;
            chirp_phase += tau * (140.0 + 780.0 * p * p) * dt;
            let env = (p * 12.0).min(1.0) * (0.25 + 0.75 * p.powf(1.4));
            let wave = (chirp_phase.sin() * 0.75 + (chirp_phase * 2.0).sin() * 0.25) * env * 0.55;
            samples.push((quantize_pcm_sample(wave), quantize_pcm_sample(wave * 0.94)));
            continue;
        }
        let v = (u - merge_u) / (1.0 - merge_u);
        let t_post = v * 1.82;
        lp_noise += (white - lp_noise) * 0.045;
        let grb_snap = white * (-t_post * 24.0).exp() * 0.55;
        let rumble = lp_noise * (-t_post * 2.1).exp() * 0.52;
        let ring_env = (-t_post * 2.4).exp() * 0.36;
        let bell_l = ((tau * 520.0 * t_post).sin() * 0.50
            + (tau * 780.0 * t_post).sin() * 0.32
            + (tau * 1040.0 * t_post).sin() * 0.18)
            * ring_env;
        let bell_r = ((tau * 529.0 * t_post).sin() * 0.50
            + (tau * 792.0 * t_post).sin() * 0.32
            + (tau * 1054.0 * t_post).sin() * 0.18)
            * ring_env;
        samples.push((
            quantize_pcm_sample((grb_snap + rumble + bell_l).clamp(-1.0, 1.0) * 0.72),
            quantize_pcm_sample((grb_snap * 0.8 + rumble + bell_r).clamp(-1.0, 1.0) * 0.72),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a 2.0-second blooming protostar main-sequence ignition harmonic swell and solar wind.
pub fn build_star_ignition_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = ((sample_rate as f32) * 2.0_f32) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let dt = 1.0 / (sample_rate as f32);
    let tau = std::f32::consts::TAU;
    let mut rng_state: u32 = 0x3A19_B7C1;
    let mut lp_noise = 0.0_f32;

    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let t = (i as f32) * dt;
        let white = next_white_noise(&mut rng_state);
        lp_noise += (white - lp_noise) * 0.06;
        let swell = (u / 0.35).min(1.0).powf(1.3);
        let tail = (-((u - 0.35).max(0.0)) * 2.4).exp();
        let env = swell * tail;

        let chord_l = 0.42 * (tau * 110.0 * t).sin()
            + 0.28 * (tau * 165.0 * t).sin()
            + 0.18 * (tau * 220.0 * t).sin()
            + 0.12 * (tau * 277.2 * t).sin();
        let chord_r = 0.42 * (tau * 110.4 * t).sin()
            + 0.28 * (tau * 165.3 * t + 0.4).sin()
            + 0.18 * (tau * 220.0 * t + 0.8).sin()
            + 0.12 * (tau * 277.8 * t).sin();
        let wind = lp_noise * (-(u - 0.35).powi(2) / 0.04).exp() * 0.35;

        samples.push((
            quantize_pcm_sample((chord_l * env * 0.60 + wind).clamp(-1.0, 1.0) * 0.72),
            quantize_pcm_sample((chord_r * env * 0.60 + wind * 0.9).clamp(-1.0, 1.0) * 0.72),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a 0.85-second space probe launch confirmation telemetry beep + sweeping thruster ignition.
pub fn build_probe_launch_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = ((sample_rate as f32) * 0.85_f32) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let dt = 1.0 / (sample_rate as f32);
    let tau = std::f32::consts::TAU;
    let mut turb_phase = 0.0_f32;
    let mut rng_state: u32 = 0x7E42_C019;
    let mut lp_noise = 0.0_f32;

    for i in 0..total_samples {
        let u = (i as f32) / (total_samples.max(1) as f32);
        let t = (i as f32) * dt;
        let white = next_white_noise(&mut rng_state);
        lp_noise += (white - lp_noise) * (0.04 + 0.22 * u).clamp(0.04, 0.28);

        let beep = if u < 0.075 {
            (tau * 880.0 * t).sin() * (1.0 - (u / 0.075).powi(2)) * 0.35
        } else if (0.095..0.165).contains(&u) {
            let bp = (u - 0.095) / 0.070;
            (tau * 1320.0 * t).sin() * (1.0 - bp * bp) * 0.38
        } else {
            0.0
        };

        let thrust_u = ((u - 0.14) / 0.86).clamp(0.0, 1.0);
        turb_phase += tau * (220.0 + 420.0 * thrust_u) * dt;
        let thrust_env = if u >= 0.14 {
            (thrust_u * 8.0).min(1.0) * (-thrust_u * 2.8).exp()
        } else {
            0.0
        };
        let thrust = (lp_noise * 0.72 + turb_phase.sin() * 0.28) * thrust_env * 0.62;
        let pan_r = 0.25 + 0.50 * thrust_u;
        let pan_l = 1.0 - pan_r;

        samples.push((
            quantize_pcm_sample(((beep + thrust * pan_l * 1.4).clamp(-1.0, 1.0)) * 0.70),
            quantize_pcm_sample(((beep + thrust * pan_r * 1.4).clamp(-1.0, 1.0)) * 0.70),
        ));
    }
    synthesize_pcm_wav_stereo(sample_rate, &samples)
}

/// Synthesizes a seamless 3.0-second stereo atmospheric canyon wind loop for First-Person Surface Mode.
pub fn build_surface_wind_loop_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = (sample_rate as usize) * 3;
    let overlap_len = (sample_rate as usize) / 2;
    let mut raw = Vec::with_capacity(total_samples + overlap_len);
    let tau = std::f32::consts::TAU;
    let dt = 1.0 / (sample_rate as f32);
    let mut rng_state: u32 = 0x19B4_8E2F;
    let (mut low_l, mut low_r, mut brown_l, mut brown_r) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);

    for i in 0..(total_samples + overlap_len) {
        let phase_lfo = (i as f32) / (total_samples as f32);
        let t = (i as f32) * dt;
        low_l += (next_white_noise(&mut rng_state) - low_l) * 0.06;
        brown_l += (low_l - brown_l) * 0.04;
        low_r += (next_white_noise(&mut rng_state) - low_r) * 0.06;
        brown_r += (low_r - brown_r) * 0.04;

        let gust_l = 0.58 + 0.28 * (tau * phase_lfo).sin() + 0.14 * (tau * 2.0 * phase_lfo).cos();
        let gust_r = 0.58
            + 0.28 * (tau * phase_lfo + 1.1).sin()
            + 0.14 * (tau * 2.0 * phase_lfo + 0.6).cos();
        let whistle_f = 310.0 + 30.0 * (tau * phase_lfo).sin();
        let whistle = (tau * whistle_f * t).sin() * 0.05;
        raw.push((
            (brown_l * 2.4 + low_l * 0.45) * gust_l + whistle,
            (brown_r * 2.4 + low_r * 0.45) * gust_r - whistle * 0.7,
        ));
    }
    let frames = crossfade_seamless_stereo_loop(&raw, total_samples, overlap_len, 0.46);
    synthesize_pcm_wav_stereo(sample_rate, &frames)
}

/// Synthesizes a seamless 3.0-second stereo rolling planetary ocean surf loop for First-Person Surface Mode.
pub fn build_surface_ocean_loop_wav(sample_rate: u32) -> Vec<u8> {
    if sample_rate == 0 {
        return synthesize_pcm_wav_stereo(sample_rate, &[]);
    }
    let total_samples = (sample_rate as usize) * 3;
    let overlap_len = (sample_rate as usize) / 2;
    let mut raw = Vec::with_capacity(total_samples + overlap_len);
    let tau = std::f32::consts::TAU;
    let mut rng_state: u32 = 0x8C3D_21F4;
    let (mut deep_l, mut deep_r, mut foam_l, mut foam_r) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);

    for i in 0..(total_samples + overlap_len) {
        let phase = (i as f32) / (total_samples as f32);
        let w_l = next_white_noise(&mut rng_state);
        let w_r = next_white_noise(&mut rng_state);
        deep_l += (w_l - deep_l) * 0.025;
        deep_r += (w_r - deep_r) * 0.025;
        foam_l += (w_l - foam_l) * 0.19;
        foam_r += (w_r - foam_r) * 0.19;

        let swell_l = 0.5 - 0.5 * (tau * 2.0 * phase).cos();
        let swell_r = 0.5 - 0.5 * (tau * 2.0 * phase + 0.55).cos();
        let body_l = deep_l * (0.45 + 1.45 * swell_l) + foam_l * swell_l.powf(1.8) * 0.48;
        let body_r = deep_r * (0.45 + 1.45 * swell_r) + foam_r * swell_r.powf(1.8) * 0.48;
        raw.push((body_l, body_r));
    }
    let frames = crossfade_seamless_stereo_loop(&raw, total_samples, overlap_len, 0.50);
    synthesize_pcm_wav_stereo(sample_rate, &frames)
}
