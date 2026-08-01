use std::num::NonZero;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rodio::source::Source;

use super::engine::{SAMPLE_RATE, SimpleRng, SoundPriority, queue_spatial};
use super::tone::{note_envelope, sweep_envelope, warm_at};
use crate::{BuildingSoundKind, SpatialSoundParams};

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

// Original mobile-UI motifs in lower register (220–440 Hz)
const CITY_MELODY: [f32; 6] = [261.63, 293.66, 329.63, 349.23, 329.63, 293.66];
const BUNKER_MELODY: [f32; 6] = [220.00, 246.94, 220.00, 196.00, 220.00, 246.94];
const FACTORY_MELODY: [f32; 6] = [293.66, 329.63, 369.99, 329.63, 293.66, 261.63];
const PORT_MELODY: [f32; 6] = [329.63, 369.99, 392.00, 369.99, 329.63, 293.66];

struct BuildingPlacementParams {
    freq: f32,
    decay_rate: f32,
    duration_secs: f32,
    amplitude: f32,
}

fn placement_params(kind: BuildingSoundKind) -> BuildingPlacementParams {
    let freq = advance_building_melody(kind);
    match kind {
        BuildingSoundKind::City => BuildingPlacementParams {
            freq,
            decay_rate: 8.0,
            duration_secs: 0.14,
            amplitude: 0.15,
        },
        BuildingSoundKind::Bunker => BuildingPlacementParams {
            freq,
            decay_rate: 7.0,
            duration_secs: 0.16,
            amplitude: 0.14,
        },
        BuildingSoundKind::Factory => BuildingPlacementParams {
            freq,
            decay_rate: 12.0,
            duration_secs: 0.11,
            amplitude: 0.14,
        },
        BuildingSoundKind::Port => BuildingPlacementParams {
            freq,
            decay_rate: 6.5,
            duration_secs: 0.13,
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
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let wave_val = warm_at(self.freq, t);
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.006, 0.03);
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
        let amp = 0.15;

        let note_dur = 0.07;
        let freqs = [523.25, 659.25, 783.99, 1046.50];
        let note_idx = (t / note_dur) as usize;
        if note_idx < freqs.len() {
            let freq = freqs[note_idx];
            let t_note = t - note_idx as f32 * note_dur;
            let wave = warm_at(freq, t_note);
            let envelope = note_envelope(t_note, note_dur, 8.0, 0.005, 0.02);
            val = wave * envelope;
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
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples as f32;

        let base_freq = 180.0 + 340.0 * progress;
        let fm = (2.0 * std::f32::consts::PI * 35.0 * t).sin() * 12.0;
        let freq = (base_freq + fm).max(20.0);
        let wave_val = warm_at(freq, t);

        let envelope = sweep_envelope(t, duration, 2.5, 0.01, 0.15);

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

        let low_freq = 80.0 * (-3.5 * t).exp() + 35.0;
        let mid_freq = 180.0 * (-5.0 * t).exp() + 60.0;
        let low = warm_at(low_freq, t);
        let mid = warm_at(mid_freq, t) * 0.45;

        let val = low * 0.65 + mid * 0.35;

        let mut envelope = (-2.2 * t).exp();
        let fade_start = duration_secs - 0.2;
        if t > fade_start {
            let linear_fade = (duration_secs - t) / 0.2;
            envelope *= linear_fade.clamp(0.0, 1.0);
        }

        self.sample_idx += 1;
        Some(val * envelope * 0.24)
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
    fn new(start_freq: f32, end_freq: f32, seed: u32) -> Self {
        let mut rng = SimpleRng::new(seed);
        let duration = rng.range(0.12, 0.18); // shorter, cleaner sweeps
        let decay_rate = rng.range(8.0, 12.0);
        let amplitude = rng.range(0.14, 0.18); // slightly lower volume for elegance

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
        let wave_val = warm_at(freq, t);
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.005, 0.04);

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

pub fn play_building_placement_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    queue_spatial(
        BuildingPlacementSource::new(kind),
        spatial,
        SoundPriority::Normal,
    );
}

pub fn play_building_completed_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
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
    // ponytail: global rate limiting and pentatonic octave-sweeps to eliminate sound spam and 8-bit aesthetic
    let now = super::engine::now_ms();
    let last = super::engine::LAST_BUNKER_SOUND_MS.load(std::sync::atomic::Ordering::Relaxed);
    if now.saturating_sub(last) < 150 {
        return;
    }
    super::engine::LAST_BUNKER_SOUND_MS.store(now, std::sync::atomic::Ordering::Relaxed);

    let SpatialSoundParams { wx, wy, .. } = spatial;
    let session = super::music::music_session()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut rng = SimpleRng::new(seed ^ super::music::tile_hash(wx, wy));
    let base_degree = super::music::pick_base_degree(&session, wx, wy, &mut rng);
    let start_freq = super::music::freq_at(session.root_octave + 1, base_degree);
    let end_freq = super::music::freq_at(session.root_octave, base_degree);

    queue_spatial(
        BunkerDefenseSource::new(start_freq, end_freq, seed),
        spatial,
        SoundPriority::Normal,
    );
}
