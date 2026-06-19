use std::num::NonZero;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rodio::source::Source;

use crate::{BuildingSoundKind, SpatialSoundParams};
use super::engine::{queue_spatial, SimpleRng, SoundPriority, SAMPLE_RATE};


#[derive(Default)]
struct BuildingSession {
    city_step: u8,
    bunker_step: u8,
    factory_step: u8,
    port_step: u8,
    last_placement: Option<Instant>,
}

static BUILDING_SESSION: OnceLock<Mutex<BuildingSession>> = OnceLock::new();

fn building_session() -> &'static Mutex<BuildingSession> {
    BUILDING_SESSION.get_or_init(|| Mutex::new(BuildingSession::default()))
}

const MELODY_RESET: Duration = Duration::from_secs(10);

// Funky Town riff fragment (City)
const CITY_MELODY: [f32; 8] = [
    392.00, 392.00, 349.23, 293.66, 261.63, 293.66, 261.63, 233.08,
];
// Imperial March opening (Bunker)
const BUNKER_MELODY: [f32; 8] = [
    293.66, 293.66, 293.66, 233.08, 349.23, 233.08, 349.23, 587.33,
];
// Axel F lead (Factory)
const FACTORY_MELODY: [f32; 8] = [
    554.37, 554.37, 466.16, 554.37, 466.16, 415.30, 369.99, 415.30,
];
// Sailor's Hornpipe opening (Port)
const PORT_MELODY: [f32; 8] = [
    392.00, 493.88, 587.33, 493.88, 392.00, 440.00, 493.88, 392.00,
];

#[derive(Clone, Copy)]
enum BuildingWaveKind {
    Pulse,
    Sawtooth,
    Triangle,
}

struct BuildingPlacementParams {
    freq: f32,
    wave: BuildingWaveKind,
    duty: f32,
    decay_rate: f32,
    duration_secs: f32,
    amplitude: f32,
}

fn placement_params(kind: BuildingSoundKind) -> BuildingPlacementParams {
    let freq = advance_building_melody(kind);
    match kind {
        BuildingSoundKind::City => BuildingPlacementParams {
            freq,
            wave: BuildingWaveKind::Pulse,
            duty: 0.25,
            decay_rate: 10.0,
            duration_secs: 0.12,
            amplitude: 0.15,
        },
        BuildingSoundKind::Bunker => BuildingPlacementParams {
            freq,
            wave: BuildingWaveKind::Sawtooth,
            duty: 0.50,
            decay_rate: 8.0,
            duration_secs: 0.15,
            amplitude: 0.14,
        },
        BuildingSoundKind::Factory => BuildingPlacementParams {
            freq,
            wave: BuildingWaveKind::Pulse,
            duty: 0.125,
            decay_rate: 22.0,
            duration_secs: 0.08,
            amplitude: 0.14,
        },
        BuildingSoundKind::Port => BuildingPlacementParams {
            freq,
            wave: BuildingWaveKind::Triangle,
            duty: 0.50,
            decay_rate: 6.0,
            duration_secs: 0.10,
            amplitude: 0.13,
        },
    }
}

fn advance_building_melody(kind: BuildingSoundKind) -> f32 {
    let mut session = building_session().lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if session
        .last_placement
        .is_some_and(|t| now.duration_since(t) > MELODY_RESET)
    {
        session.city_step = 0;
        session.bunker_step = 0;
        session.factory_step = 0;
        session.port_step = 0;
    }
    session.last_placement = Some(now);

    let (melody, step) = match kind {
        BuildingSoundKind::City => (&CITY_MELODY, &mut session.city_step),
        BuildingSoundKind::Bunker => (&BUNKER_MELODY, &mut session.bunker_step),
        BuildingSoundKind::Factory => (&FACTORY_MELODY, &mut session.factory_step),
        BuildingSoundKind::Port => (&PORT_MELODY, &mut session.port_step),
    };
    let idx = (*step as usize) % melody.len();
    let freq = melody[idx];
    *step = step.wrapping_add(1);
    freq
}
struct BuildingPlacementSource {
    sample_idx: u64,
    freq: f32,
    wave: BuildingWaveKind,
    duty: f32,
    decay_rate: f32,
    amplitude: f32,
    duration_samples: u64,
}

impl BuildingPlacementSource {
    fn new(kind: BuildingSoundKind) -> Self {
        let p = placement_params(kind);
        Self {
            sample_idx: 0,
            freq: p.freq,
            wave: p.wave,
            duty: p.duty,
            decay_rate: p.decay_rate,
            amplitude: p.amplitude,
            duration_samples: (SAMPLE_RATE as f32 * p.duration_secs) as u64,
        }
    }
}

impl Iterator for BuildingPlacementSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let period = 1.0 / self.freq.max(20.0);
        let phase = (t % period) / period;
        let wave_val = match self.wave {
            BuildingWaveKind::Pulse => {
                if phase < self.duty {
                    1.0
                } else {
                    -1.0
                }
            }
            BuildingWaveKind::Sawtooth => 2.0 * phase - 1.0,
            BuildingWaveKind::Triangle => 2.0 * (2.0 * phase - 1.0).abs() - 1.0,
        };
        let envelope = (-self.decay_rate * t).exp();
        self.sample_idx += 1;
        Some(wave_val * envelope * self.amplitude)
    }
}

impl Source for BuildingPlacementSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(1).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.duration_samples as f32 / SAMPLE_RATE as f32,
        ))
    }
}

struct BuildingCompletionSource {
    sample_idx: u64,
    duration_samples: u64,
}

impl BuildingCompletionSource {
    fn new() -> Self {
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * 0.28) as u64,
        }
    }
}

impl Iterator for BuildingCompletionSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let mut val = 0.0_f32;
        let amp = 0.16;

        // ponytail: play the standard bright completion arpeggio for all buildings
        let note_dur = 0.07;
        let freqs = [523.25, 659.25, 783.99, 1046.50];
        let note_idx = (t / note_dur) as usize;
        if note_idx < freqs.len() {
            let freq = freqs[note_idx];
            let t_note = t - note_idx as f32 * note_dur;
            let period = 1.0 / freq;
            let phase = (t_note % period) / period;
            let wave = if phase < 0.25 { 1.0 } else { -1.0 };
            val = wave * (-10.0 * t_note).exp();
        }

        self.sample_idx += 1;
        Some(val * amp)
    }
}

impl Source for BuildingCompletionSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(1).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.duration_samples as f32 / SAMPLE_RATE as f32,
        ))
    }
}

struct NukeLaunchSource {
    sample_idx: u64,
    duration_samples: u64,
}

impl NukeLaunchSource {
    fn new() -> Self {
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * 1.2) as u64,
        }
    }
}

impl Iterator for NukeLaunchSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples as f32;

        let base_freq = 150.0 + 700.0 * progress;
        let fm = (2.0 * std::f32::consts::PI * 45.0 * t).sin() * 25.0;
        let freq = (base_freq + fm).max(20.0);

        let period = 1.0 / freq;
        let phase = (t % period) / period;
        let wave_val = if phase < 0.25 { 1.0 } else { -1.0 };

        let mut envelope = 1.0;
        let fade_start = 1.2 - 0.15;
        if t > fade_start {
            envelope = ((1.2 - t) / 0.15).clamp(0.0, 1.0);
        }

        self.sample_idx += 1;
        Some(wave_val * envelope * 0.18)
    }
}

impl Source for NukeLaunchSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(1).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.duration_samples as f32 / SAMPLE_RATE as f32,
        ))
    }
}

struct NukeImpactSource {
    sample_idx: u64,
    duration_samples: u64,
    level: u8,
}

impl NukeImpactSource {
    fn new(level: u8) -> Self {
        let duration_secs = 1.2 + (level as f32 * 0.3);
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * duration_secs) as u64,
            level,
        }
    }
}

impl Iterator for NukeImpactSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration_secs = 1.2 + (self.level as f32 * 0.3);

        let freq = 120.0 * (-4.0 * t).exp() + 20.0;
        let fm = (2.0 * std::f32::consts::PI * 150.0 * t).sin() * 8.0;

        let period_saw = 1.0 / (freq + fm).max(20.0);
        let phase_saw = (t % period_saw) / period_saw;
        let sawtooth_val = 2.0 * phase_saw - 1.0;

        let period_tri = 1.0 / (freq * 0.5).max(10.0);
        let phase_tri = (t % period_tri) / period_tri;
        let triangle_val = 2.0 * (2.0 * phase_tri - 1.0).abs() - 1.0;

        let val = sawtooth_val * 0.7 + triangle_val * 0.3;

        let mut envelope = (-2.5 * t).exp();
        let fade_start = duration_secs - 0.2;
        if t > fade_start {
            let linear_fade = (duration_secs - t) / 0.2;
            envelope *= linear_fade.clamp(0.0, 1.0);
        }

        self.sample_idx += 1;
        Some(val * envelope * 0.25)
    }
}

impl Source for NukeImpactSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(1).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.duration_samples as f32 / SAMPLE_RATE as f32,
        ))
    }
}

struct BunkerDefenseSource {
    sample_idx: u64,
    duration_samples: u64,
    start_freq: f32,
    end_freq: f32,
    decay_rate: f32,
    amplitude: f32,
}

impl BunkerDefenseSource {
    fn new(seed: u32) -> Self {
        let mut rng = SimpleRng::new(seed);
        let duration = rng.range(0.15, 0.22);
        // ponytail: lower, warmer frequencies for a modern mobile RTS vibe (Rise of Kingdoms style)
        let start_freq = rng.range(350.0, 480.0);
        let end_freq = rng.range(120.0, 200.0);
        let decay_rate = rng.range(8.0, 12.0);
        let amplitude = rng.range(0.20, 0.25); // slightly louder since sine is softer than square

        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * duration) as u64,
            start_freq,
            end_freq,
            decay_rate,
            amplitude,
        }
    }
}

impl Iterator for BunkerDefenseSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples as f32;

        let freq = self.start_freq + (self.end_freq - self.start_freq) * progress;

        // ponytail: warm harmonic sine wave combination instead of harsh 8-bit square wave
        let angle = 2.0 * std::f32::consts::PI * t * freq;
        let wave_val = angle.sin() * 0.7 + (2.0 * angle).sin() * 0.25 + (3.0 * angle).sin() * 0.05;

        let mut envelope = (-self.decay_rate * t).exp();
        let fade_start = duration - 0.04;
        if t > fade_start {
            let linear_fade = (duration - t) / 0.04;
            envelope *= linear_fade.clamp(0.0, 1.0);
        }

        self.sample_idx += 1;
        Some(wave_val * envelope * self.amplitude)
    }
}

impl Source for BunkerDefenseSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(1).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.duration_samples as f32 / SAMPLE_RATE as f32,
        ))
    }
}
pub fn play_building_placement_sound(
    kind: BuildingSoundKind,
    spatial: SpatialSoundParams,
) {
    queue_spatial(
        BuildingPlacementSource::new(kind),
        spatial,
        SoundPriority::Normal,
    );
}

pub fn play_building_completed_sound(
    kind: BuildingSoundKind,
    spatial: SpatialSoundParams,
) {
    let _ = kind;
    queue_spatial(
        BuildingCompletionSource::new(),
        spatial,
        SoundPriority::Foreground,
    );
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    queue_spatial(NukeLaunchSource::new(), spatial, SoundPriority::Foreground);
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    queue_spatial(
        NukeImpactSource::new(level),
        spatial,
        SoundPriority::Foreground,
    );
}

pub fn play_bunker_defense_sound(seed: u32, spatial: SpatialSoundParams) {
    queue_spatial(
        BunkerDefenseSource::new(seed),
        spatial,
        SoundPriority::Normal,
    );
}
