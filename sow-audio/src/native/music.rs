use std::sync::{Mutex, OnceLock};

use super::engine::{ArpeggioSource, play_ui, SimpleRng, SAMPLE_RATE};

pub(super) struct MusicSession {
    pub(super) root_degree: u8,
    pub(super) root_octave: u8,
    pub(super) phrase_step: u32,
    pub(super) last_degree: u8,
}

impl Default for MusicSession {
    fn default() -> Self {
        Self {
            root_degree: 0,
            root_octave: 3,
            phrase_step: 0,
            last_degree: 0,
        }
    }
}

static MUSIC_SESSION: OnceLock<Mutex<MusicSession>> = OnceLock::new();

pub(super) fn music_session() -> &'static Mutex<MusicSession> {
    MUSIC_SESSION.get_or_init(|| Mutex::new(MusicSession::default()))
}
pub fn set_music_context(seed: u32, anchor_wx: f32, anchor_wy: f32) {
    let mixed = seed ^ tile_hash(anchor_wx, anchor_wy);
    let mut session = music_session().lock().unwrap_or_else(|e| e.into_inner());
    session.root_degree = (mixed % 5) as u8;
    session.root_octave = 2 + ((mixed >> 8) % 3) as u8;
    session.phrase_step = 0;
    session.last_degree = session.root_degree;
}
pub(super) const PENTATONIC_SCALE: [f32; 15] = [
    110.00, 130.81, 146.83, 164.81, 196.00, 220.00, 261.63, 293.66, 329.63, 392.00, 440.00,
    523.25, 587.33, 659.25, 783.99,
];

pub(super) fn tile_hash(wx: f32, wy: f32) -> u32 {
    let ix = wx.floor() as u32;
    let iy = wy.floor() as u32;
    ix.wrapping_mul(73856093) ^ iy.wrapping_mul(19349663)
}

pub(super) fn scale_index(octave: u8, degree: u8) -> usize {
    let o = (octave as usize).saturating_sub(2);
    (o * 5 + degree as usize).min(PENTATONIC_SCALE.len() - 1)
}

pub(super) fn freq_at(octave: u8, degree: u8) -> f32 {
    PENTATONIC_SCALE[scale_index(octave, degree.min(4))]
}

pub(super) fn degrees_to_freqs(octave: u8, degrees: &[u8; 4]) -> [f32; 4] {
    [
        freq_at(octave, degrees[0]),
        freq_at(octave, degrees[1]),
        freq_at(octave, degrees[2]),
        freq_at(octave, degrees[3]),
    ]
}

pub(super) fn pick_base_degree(session: &MusicSession, wx: f32, wy: f32, rng: &mut SimpleRng) -> u8 {
    let raw = ((tile_hash(wx, wy)
        .wrapping_add(session.phrase_step)
        .wrapping_add(rng.next_u32() % 3))
        % 5) as u8;
    let last = session.last_degree;
    if raw > last.saturating_add(2) {
        raw.saturating_sub(1).min(4)
    } else if raw.saturating_add(2) < last {
        (raw + 1).min(4)
    } else {
        raw
    }
}

pub(super) fn note_dur_samples(secs: f32) -> u32 {
    (SAMPLE_RATE as f32 * secs) as u32
}
fn build_victory_sound() -> ArpeggioSource {
    let note_freqs = [523.25, 659.25, 783.99, 1046.50];
    let note_dur_samples = (SAMPLE_RATE as f32 * 0.15) as u32;
    let note_durations = [note_dur_samples; 4];
    ArpeggioSource::new(note_freqs, note_durations, 5.5, 0.15)
}

fn build_defeat_sound() -> ArpeggioSource {
    let note_freqs = [261.63, 196.00, 164.81, 130.81];
    let note_dur_samples = (SAMPLE_RATE as f32 * 0.28) as u32;
    let note_durations = [note_dur_samples; 4];
    ArpeggioSource::new(note_freqs, note_durations, 3.0, 0.12)
}

pub fn play_victory_sound() {
    play_ui(build_victory_sound());
}

pub fn play_defeat_sound() {
    play_ui(build_defeat_sound());
}
