use std::num::NonZero;
use std::time::Duration;

use rodio::source::Source;

use crate::{PlayerSoundType, SpatialSoundParams};
use super::engine::{ArpeggioSource, SimpleRng, queue_spatial, SoundPriority, SAMPLE_RATE};
use super::music::{
    degrees_to_freqs, music_session, note_dur_samples, pick_base_degree, tile_hash, MusicSession,
};
use super::tone::{sweep_envelope, warm_at};


pub(super) fn build_death_sound(
    session: &mut MusicSession,
    player_type: PlayerSoundType,
    seed: u32,
    wx: f32,
    wy: f32,
) -> ArpeggioSource {
    let mut rng = SimpleRng::new(seed ^ tile_hash(wx, wy));
    let base_degree = pick_base_degree(session, wx, wy, &mut rng);
    let note_count = match player_type {
        PlayerSoundType::Human => 4,
        PlayerSoundType::Nation => 3,
        PlayerSoundType::Bot => 3,
    };
    let octave = session.root_octave.saturating_sub(1).max(2);
    let mut degrees = [0u8; 4];
    for (i, degree) in degrees.iter_mut().enumerate().take(note_count) {
        *degree = base_degree.saturating_sub(i as u8);
    }
    let note_freqs = degrees_to_freqs(octave, &degrees);
    let base_dur = match player_type {
        PlayerSoundType::Human => 0.10,
        PlayerSoundType::Nation => 0.14,
        PlayerSoundType::Bot => 0.08,
    };
    let note_dur_secs = base_dur * rng.range(0.9, 1.1);
    let note_dur_samples = note_dur_samples(note_dur_secs);
    let note_durations = [
        note_dur_samples,
        note_dur_samples,
        note_dur_samples,
        if note_count >= 4 { note_dur_samples } else { 0 },
    ];
    let decay_rate = match player_type {
        PlayerSoundType::Human => 6.0,
        PlayerSoundType::Nation => 4.5,
        PlayerSoundType::Bot => 8.0,
    } * rng.range(0.9, 1.1);
    let amplitude = 0.12 * rng.range(0.9, 1.1);
    session.last_degree = base_degree;
    session.phrase_step = session.phrase_step.wrapping_add(1);
    ArpeggioSource::new(note_freqs, note_durations, decay_rate, amplitude)
}

pub(super) struct PulseSource {
    sample_idx: u64,
    freq: f32,
    decay_rate: f32,
    pub(super) duration_samples: u64,
    amplitude: f32,
}

impl PulseSource {
    pub(super) fn new(freq: f32, duration_secs: f32, decay_rate: f32, amplitude: f32) -> Self {
        Self {
            sample_idx: 0,
            freq: freq.max(20.0),
            decay_rate,
            duration_samples: (SAMPLE_RATE as f32 * duration_secs.max(0.01)) as u64,
            amplitude,
        }
    }
}

impl Iterator for PulseSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let val = warm_at(self.freq, t);
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.005, 0.02);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl Source for PulseSource {
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

pub fn play_death_sound(
    player_type: PlayerSoundType,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    let SpatialSoundParams { wx, wy, .. } = spatial;
    let source = {
        let mut session = music_session().lock().unwrap_or_else(|e| e.into_inner());
        build_death_sound(&mut session, player_type, seed, wx, wy)
    };
    queue_spatial(source, spatial, SoundPriority::Foreground);
}
