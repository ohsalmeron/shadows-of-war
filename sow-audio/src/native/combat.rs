use std::num::NonZero;
use std::time::Duration;

use rodio::source::Source;

use super::death::PulseSource;
use super::engine::{ArpeggioSource, SAMPLE_RATE, SimpleRng, SoundPriority, queue_spatial};
use super::music::{
    MusicSession, degrees_to_freqs, freq_at, music_session, note_dur_samples, pick_base_degree,
    tile_hash,
};
use super::tone::{sweep_envelope, warm_at};
use crate::{CombatSoundKind, SpatialSoundParams};

struct DoublePulseSource {
    pulse1: PulseSource,
    pulse2: PulseSource,
    silence_samples: u64,
    sample_idx: u64,
}

impl DoublePulseSource {
    fn new(p1: PulseSource, p2: PulseSource, gap_secs: f32) -> Self {
        Self {
            pulse1: p1,
            pulse2: p2,
            silence_samples: (SAMPLE_RATE as f32 * gap_secs) as u64,
            sample_idx: 0,
        }
    }
}

impl Iterator for DoublePulseSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx < self.pulse1.duration_samples {
            self.sample_idx += 1;
            self.pulse1.next()
        } else if self.sample_idx < self.pulse1.duration_samples + self.silence_samples {
            self.sample_idx += 1;
            Some(0.0)
        } else {
            self.sample_idx += 1;
            self.pulse2.next()
        }
    }
}

impl Source for DoublePulseSource {
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
        let total_samples =
            self.pulse1.duration_samples + self.silence_samples + self.pulse2.duration_samples;
        Some(Duration::from_secs_f32(
            total_samples as f32 / SAMPLE_RATE as f32,
        ))
    }
}

struct WarHornSource {
    sample_idx: u64,
    duration_samples: u64,
    base_freq: f32,
    vibrato_freq: f32,
    vibrato_depth: f32,
    decay_rate: f32,
    amplitude: f32,
}

impl WarHornSource {
    fn new(freq: f32, dur: f32, v_freq: f32, v_depth: f32, decay: f32, amp: f32) -> Self {
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * dur) as u64,
            base_freq: freq,
            vibrato_freq: v_freq,
            vibrato_depth: v_depth,
            decay_rate: decay,
            amplitude: amp,
        }
    }
}

impl Iterator for WarHornSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let vibrato =
            (2.0 * std::f32::consts::PI * self.vibrato_freq * t).sin() * self.vibrato_depth;
        let freq = (self.base_freq + vibrato).max(20.0);
        let val = warm_at(freq, t);
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.008, 0.025);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl Source for WarHornSource {
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

struct SweepSource {
    sample_idx: u64,
    start_freq: f32,
    end_freq: f32,
    decay_rate: f32,
    duration_samples: u64,
    amplitude: f32,
}

impl SweepSource {
    fn new(
        start_freq: f32,
        end_freq: f32,
        duration_secs: f32,
        decay_rate: f32,
        amplitude: f32,
    ) -> Self {
        Self {
            sample_idx: 0,
            start_freq: start_freq.max(20.0),
            end_freq: end_freq.max(20.0),
            decay_rate,
            duration_samples: (SAMPLE_RATE as f32 * duration_secs.max(0.01)) as u64,
            amplitude,
        }
    }
}

impl Iterator for SweepSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples.max(1) as f32;
        let freq = self.start_freq + (self.end_freq - self.start_freq) * progress;
        let val = warm_at(freq, t);
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.005, 0.03);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl Source for SweepSource {
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

struct DualSweepSource {
    sweep1: SweepSource,
    sweep2: SweepSource,
}

impl DualSweepSource {
    fn new(s1: SweepSource, s2: SweepSource) -> Self {
        Self {
            sweep1: s1,
            sweep2: s2,
        }
    }
}

impl Iterator for DualSweepSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        let v1 = self.sweep1.next();
        let v2 = self.sweep2.next();
        match (v1, v2) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

impl Source for DualSweepSource {
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
        let d1 = self.sweep1.total_duration().unwrap_or(Duration::ZERO);
        let d2 = self.sweep2.total_duration().unwrap_or(Duration::ZERO);
        Some(d1.max(d2))
    }
}

struct DeploySource {
    sample_idx: u64,
    duration_samples: u64,
    start_freq: f32,
    end_freq: f32,
    amplitude: f32,
}

impl DeploySource {
    fn new() -> Self {
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * 0.12) as u64,
            start_freq: 200.0,
            end_freq: 450.0,
            amplitude: 0.17,
        }
    }
}

impl Iterator for DeploySource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples.max(1) as f32;
        let freq = self.start_freq + (self.end_freq - self.start_freq) * progress;
        let val = warm_at(freq, t);
        let envelope = sweep_envelope(t, duration, 10.0, 0.006, 0.025);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl Source for DeploySource {
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

enum ProceduralSound {
    Pulse(PulseSource),
    Arpeggio(ArpeggioSource),
    DoublePulse(DoublePulseSource),
    WarHorn(WarHornSource),
    Sweep(SweepSource),
    DualSweep(DualSweepSource),
}

impl Iterator for ProceduralSound {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Pulse(s) => s.next(),
            Self::Arpeggio(s) => s.next(),
            Self::DoublePulse(s) => s.next(),
            Self::WarHorn(s) => s.next(),
            Self::Sweep(s) => s.next(),
            Self::DualSweep(s) => s.next(),
        }
    }
}

impl Source for ProceduralSound {
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
        match self {
            Self::Pulse(s) => s.total_duration(),
            Self::Arpeggio(s) => s.total_duration(),
            Self::DoublePulse(s) => s.total_duration(),
            Self::WarHorn(s) => s.total_duration(),
            Self::Sweep(s) => s.total_duration(),
            Self::DualSweep(s) => s.total_duration(),
        }
    }
}

fn combat_amplitude(troops: f32) -> f32 {
    0.10 + 0.06 * (troops / 5000.0).clamp(0.15, 1.0).sqrt()
}

fn build_procedural_sound(
    session: &mut MusicSession,
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    wx: f32,
    wy: f32,
) -> ProceduralSound {
    let mut rng = SimpleRng::new(tile_hash(wx, wy) ^ session.phrase_step ^ seed);
    let base_degree = pick_base_degree(session, wx, wy, &mut rng);
    let octave = session.root_octave;
    let base_freq = freq_at(octave, base_degree);
    let amp = combat_amplitude(troops) * rng.range(0.9, 1.1);

    let sound = match kind {
        CombatSoundKind::WildernessExpansion => {
            let jitter = rng.range(0.95, 1.05);
            let root = base_freq * jitter;
            let troop_scale = (troops / 2000.0).clamp(0.0, 1.0);
            let start = root * (0.85 - 0.15 * troop_scale);
            let end = root * (1.25 + 0.35 * troop_scale);
            let dur = 0.07 + 0.12 * troop_scale;
            let decay = 10.0 - 3.0 * troop_scale;

            if troops > 1000.0 {
                let s1 = SweepSource::new(start, end, dur, decay, amp);
                let s2 = SweepSource::new(start * 0.5, end * 0.5, dur, decay, amp * 0.35);
                ProceduralSound::DualSweep(DualSweepSource::new(s1, s2))
            } else {
                ProceduralSound::Sweep(SweepSource::new(start, end, dur, decay, amp))
            }
        }
        CombatSoundKind::AttackHuman => {
            let jitter = rng.range(0.975, 1.025);
            let root = base_freq * jitter * 0.92;
            let dur = 0.09 * rng.range(0.9, 1.1);
            let decay = rng.range(14.0, 18.0);

            if troops > 1500.0 {
                let p1 = PulseSource::new(root, dur * 0.5, decay, amp);
                let p2 = PulseSource::new(root * 1.03, dur * 0.5, decay, amp * 0.9);
                ProceduralSound::DoublePulse(DoublePulseSource::new(p1, p2, 0.02))
            } else {
                ProceduralSound::Pulse(PulseSource::new(root, dur, decay, amp))
            }
        }
        CombatSoundKind::AttackEmpire => {
            let d1 = (base_degree + 2).min(4);
            let d2 = (base_degree + 4).min(4);
            let dur_step = 0.08 * (1.0 + (troops / 5000.0).clamp(0.0, 0.4));
            let dur = note_dur_samples(dur_step);
            let decay = rng.range(6.0, 9.0);
            ProceduralSound::Arpeggio(ArpeggioSource::new(
                degrees_to_freqs(octave, &[base_degree, d1, d2, 0]),
                [dur, dur, dur, 0],
                decay,
                amp * 1.1,
            ))
        }
        CombatSoundKind::AttackTribe => {
            let jitter = rng.range(0.92, 1.08);
            let root = base_freq * jitter * 0.88;
            let v_freq = 12.0 * rng.range(0.85, 1.15);
            let v_depth = 18.0 * rng.range(0.85, 1.15);
            let dur = 0.08 + 0.05 * (troops / 3000.0).clamp(0.0, 1.0);
            let decay = rng.range(10.0, 14.0);
            ProceduralSound::WarHorn(WarHornSource::new(root, dur, v_freq, v_depth, decay, amp))
        }
        CombatSoundKind::CounterAttack => {
            let jitter = rng.range(0.95, 1.05);
            let root = base_freq * jitter;
            let troop_scale = (troops / 3000.0).clamp(0.0, 1.0);
            let dur = 0.16 + 0.06 * troop_scale;
            let decay = 8.0 - 1.5 * troop_scale;

            let s1 = SweepSource::new(root * 1.05, root * 0.85, dur, decay, amp);
            let s2 = SweepSource::new(root * 0.75, root * 0.58, dur, decay, amp * 0.55);
            ProceduralSound::DualSweep(DualSweepSource::new(s1, s2))
        }
    };
    session.last_degree = base_degree;
    session.phrase_step = session.phrase_step.wrapping_add(1);
    sound
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    queue_spatial(DeploySource::new(), spatial, SoundPriority::Normal);
}

pub fn play_combat_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    // ponytail: global rate limiting to prune combat spam
    let now = super::engine::now_ms();
    let last = super::engine::LAST_COMBAT_SOUND_MS.load(std::sync::atomic::Ordering::Relaxed);
    if now.saturating_sub(last) < 120 {
        return;
    }
    super::engine::LAST_COMBAT_SOUND_MS.store(now, std::sync::atomic::Ordering::Relaxed);

    let SpatialSoundParams { wx, wy, .. } = spatial;
    let source = {
        let mut session = music_session().lock().unwrap_or_else(|e| e.into_inner());
        build_procedural_sound(&mut session, kind, troops, seed, wx, wy)
    };
    queue_spatial(source, spatial, SoundPriority::Background);
}
