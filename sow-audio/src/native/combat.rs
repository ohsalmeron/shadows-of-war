use std::io::Cursor;
use std::num::NonZero;
use std::time::Duration;

use rodio::source::Source;

use crate::{CombatSoundKind, SpatialSoundParams};
use super::death::PulseSource;
use super::engine::{
    ArpeggioSource, SimpleRng, queue_spatial, SoundPriority, DEPLOY_WAV, SAMPLE_RATE,
};
use super::music::{
    degrees_to_freqs, freq_at, music_session, note_dur_samples, pick_base_degree, tile_hash,
    MusicSession,
};


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
    duty: f32,
    decay_rate: f32,
    amplitude: f32,
}

impl WarHornSource {
    fn new(
        freq: f32,
        dur: f32,
        v_freq: f32,
        v_depth: f32,
        duty: f32,
        decay: f32,
        amp: f32,
    ) -> Self {
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * dur) as u64,
            base_freq: freq,
            vibrato_freq: v_freq,
            vibrato_depth: v_depth,
            duty,
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
        let vibrato =
            (2.0 * std::f32::consts::PI * self.vibrato_freq * t).sin() * self.vibrato_depth;
        let freq = (self.base_freq + vibrato).max(20.0);
        let period = 1.0 / freq;
        let phase = (t % period) / period;
        let val = if phase < self.duty { 1.0 } else { -1.0 };
        let envelope = (-self.decay_rate * t).exp();
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
    duty: f32,
    decay_rate: f32,
    duration_samples: u64,
    amplitude: f32,
}

impl SweepSource {
    fn new(
        start_freq: f32,
        end_freq: f32,
        duty: f32,
        duration_secs: f32,
        decay_rate: f32,
        amplitude: f32,
    ) -> Self {
        Self {
            sample_idx: 0,
            start_freq: start_freq.max(20.0),
            end_freq: end_freq.max(20.0),
            duty: duty.clamp(0.05, 0.95),
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
        let progress = self.sample_idx as f32 / self.duration_samples.max(1) as f32;
        let freq = self.start_freq + (self.end_freq - self.start_freq) * progress;
        let period = 1.0 / freq.max(20.0);
        let phase = (t % period) / period;
        let val = if phase < self.duty { 1.0 } else { -1.0 };
        let envelope = (-self.decay_rate * t).exp();
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
            let start = root * (0.8 - 0.2 * troop_scale);
            let end = root * (1.5 + 0.5 * troop_scale);
            let dur = 0.05 + 0.10 * troop_scale;
            let duty = 0.125 + 0.225 * troop_scale;
            let decay = 12.0 - 4.0 * troop_scale;

            if troops > 1000.0 {
                let s1 = SweepSource::new(start, end, duty, dur, decay, amp);
                let s2 = SweepSource::new(start * 0.5, end * 0.5, duty, dur, decay, amp * 0.4);
                ProceduralSound::DualSweep(DualSweepSource::new(s1, s2))
            } else {
                ProceduralSound::Sweep(SweepSource::new(start, end, duty, dur, decay, amp))
            }
        }
        CombatSoundKind::AttackHuman => {
            let jitter = rng.range(0.975, 1.025);
            let root = base_freq * jitter;
            let dur = 0.08 * rng.range(0.9, 1.1);
            let decay = rng.range(18.0, 22.0);

            if troops > 1500.0 {
                let p1 = PulseSource::new(root, 0.125, dur * 0.5, decay, amp);
                let p2 = PulseSource::new(root * 1.05, 0.125, dur * 0.5, decay, amp);
                ProceduralSound::DoublePulse(DoublePulseSource::new(p1, p2, 0.02))
            } else {
                ProceduralSound::Pulse(PulseSource::new(root, 0.125, dur, decay, amp))
            }
        }
        CombatSoundKind::AttackEmpire => {
            let d1 = (base_degree + 2).min(4);
            let d2 = (base_degree + 4).min(4);
            let dur_step = 0.06 * (1.0 + (troops / 5000.0).clamp(0.0, 0.5));
            let dur = note_dur_samples(dur_step);
            let decay = rng.range(8.0, 12.0);
            ProceduralSound::Arpeggio(ArpeggioSource::new(
                degrees_to_freqs(octave, &[base_degree, d1, d2, 0]),
                [dur, dur, dur, 0],
                0.50,
                decay,
                amp * 1.2,
            ))
        }
        CombatSoundKind::AttackTribe => {
            let jitter = rng.range(0.9, 1.1);
            let root = base_freq * jitter;
            let v_freq = 15.0 * rng.range(0.8, 1.2);
            let v_depth = 35.0 * rng.range(0.8, 1.2);
            let dur = 0.06 + 0.04 * (troops / 3000.0).clamp(0.0, 1.0);
            let decay = rng.range(14.0, 18.0);
            ProceduralSound::WarHorn(WarHornSource::new(
                root, dur, v_freq, v_depth, 0.25, decay, amp,
            ))
        }
        CombatSoundKind::CounterAttack => {
            let jitter = rng.range(0.95, 1.05);
            let root = base_freq * jitter;
            let troop_scale = (troops / 3000.0).clamp(0.0, 1.0);
            let dur = 0.14 + 0.06 * troop_scale;
            let duty = 0.125 + 0.125 * troop_scale;
            let decay = 10.0 - 2.0 * troop_scale;

            let s1 = SweepSource::new(root * 1.1, root * 0.8, duty, dur, decay, amp);
            let s2 = SweepSource::new(root * 0.7, root * 0.5, duty, dur, decay, amp * 0.6);
            ProceduralSound::DualSweep(DualSweepSource::new(s1, s2))
        }
    };
    session.last_degree = base_degree;
    session.phrase_step = session.phrase_step.wrapping_add(1);
    sound
}
pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    let cursor = Cursor::new(DEPLOY_WAV);
    if let Ok(source) = rodio::Decoder::new(cursor) {
        queue_spatial(source.amplify(0.17), spatial, SoundPriority::Normal);
    }
}

pub fn play_combat_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    let SpatialSoundParams { wx, wy, .. } = spatial;
    let source = {
        let mut session = music_session().lock().unwrap_or_else(|e| e.into_inner());
        build_procedural_sound(&mut session, kind, troops, seed, wx, wy)
    };
    queue_spatial(source, spatial, SoundPriority::Background);
}
