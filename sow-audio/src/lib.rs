//! Procedural retro audio effects with spatial panning.
//! Leaf crate: no workspace dependencies beyond rodio (native only).

/// Combat / expansion sound category for procedural synthesis.
#[derive(Clone, Copy, Debug)]
pub enum CombatSoundKind {
    WildernessExpansion,
    AttackHuman,
    AttackEmpire,
    AttackTribe,
    CounterAttack,
}

/// Player archetype for death-sound synthesis.
#[derive(Clone, Copy, Debug)]
pub enum PlayerSoundType {
    Human,
    Nation,
    Bot,
}

/// Structure type for placement-sound synthesis.
#[derive(Clone, Copy, Debug)]
pub enum BuildingSoundKind {
    City,
    Bunker,
    Factory,
    Port,
}

/// World position and camera state for spatial audio panning/attenuation.
#[derive(Clone, Copy, Debug)]
pub struct SpatialSoundParams {
    pub wx: f32,
    pub wy: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
}

pub fn play_death_sound(player_type: PlayerSoundType, seed: u32, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_death_sound(player_type, seed, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (player_type, seed, spatial);
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_deploy_sound(spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = spatial;
}

pub fn play_combat_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_combat_sound(kind, troops, seed, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (kind, troops, seed, spatial);
}

pub fn play_building_placement_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_building_placement_sound(kind, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (kind, spatial);
}

pub fn play_building_completed_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_building_completed_sound(kind, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (kind, spatial);
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_nuke_launch_sound(spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = spatial;
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_nuke_impact_sound(level, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (level, spatial);
}

pub fn play_bunker_defense_sound(seed: u32, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_bunker_defense_sound(seed, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (seed, spatial);
}

pub fn set_music_context(seed: u32, anchor_wx: f32, anchor_wy: f32) {
    #[cfg(not(target_arch = "wasm32"))]
    native::set_music_context(seed, anchor_wx, anchor_wy);
    #[cfg(target_arch = "wasm32")]
    let _ = (seed, anchor_wx, anchor_wy);
}

pub fn play_victory_sound() {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_victory_sound();
}

pub fn play_defeat_sound() {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_defeat_sound();
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::play_spatial;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io::Cursor;
    use std::num::NonZero;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use rodio::source::Source;
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

    use super::{BuildingSoundKind, CombatSoundKind, PlayerSoundType};

    const SAMPLE_RATE: u32 = 22050;
    const OPEN_BACKOFF: Duration = Duration::from_secs(2);
    const MAX_VOICES: u8 = 6;
    const DEPLOY_WAV: &[u8] = include_bytes!("../../assets/static/ui/deploy.wav");

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SoundPriority {
        Background,
        Normal,
        Foreground,
    }

    struct PlayCommand {
        source: BoxedSource,
        left: f32,
        right: f32,
        priority: SoundPriority,
        duration: Duration,
    }

    struct VoiceSlot {
        ends_at: Instant,
    }

    struct MusicSession {
        root_degree: u8,
        root_octave: u8,
        phrase_step: u32,
        last_degree: u8,
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

    fn music_session() -> &'static Mutex<MusicSession> {
        MUSIC_SESSION.get_or_init(|| Mutex::new(MusicSession::default()))
    }

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

    pub fn set_music_context(seed: u32, anchor_wx: f32, anchor_wy: f32) {
        let mixed = seed ^ tile_hash(anchor_wx, anchor_wy);
        let mut session = music_session().lock().unwrap_or_else(|e| e.into_inner());
        session.root_degree = (mixed % 5) as u8;
        session.root_octave = 2 + ((mixed >> 8) % 3) as u8;
        session.phrase_step = 0;
        session.last_degree = session.root_degree;
    }

    struct BoxedSource(Box<dyn Source<Item = f32> + Send>);

    impl Iterator for BoxedSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next()
        }
    }

    impl Source for BoxedSource {
        fn current_span_len(&self) -> Option<usize> {
            self.0.current_span_len()
        }

        fn channels(&self) -> NonZero<u16> {
            self.0.channels()
        }

        fn sample_rate(&self) -> NonZero<u32> {
            self.0.sample_rate()
        }

        fn total_duration(&self) -> Option<Duration> {
            self.0.total_duration()
        }

        fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
            self.0.try_seek(pos)
        }
    }

    struct AudioWorkerState {
        stream: Option<MixerDeviceSink>,
        open_backoff_until: Option<Instant>,
        voices: Vec<VoiceSlot>,
    }

    fn prune_voices(voices: &mut Vec<VoiceSlot>) {
        let now = Instant::now();
        voices.retain(|v| v.ends_at > now);
    }

    fn active_voice_count(voices: &[VoiceSlot]) -> u8 {
        voices.len().min(u8::MAX as usize) as u8
    }

    fn should_play(priority: SoundPriority, active: u8) -> bool {
        match priority {
            SoundPriority::Background => active < MAX_VOICES,
            SoundPriority::Normal => active < MAX_VOICES + 1,
            SoundPriority::Foreground => true,
        }
    }

    fn priority_gain(priority: SoundPriority, active: u8) -> f32 {
        let base = match priority {
            SoundPriority::Background => 0.30,
            SoundPriority::Normal => 0.70,
            SoundPriority::Foreground => 1.0,
        };
        if priority == SoundPriority::Background {
            base / (1.0 + active as f32 * 0.2)
        } else {
            base
        }
    }

    fn source_duration<S: Source<Item = f32>>(source: &S) -> Duration {
        source
            .total_duration()
            .unwrap_or(Duration::from_millis(150))
    }

    static AUDIO_TX: OnceLock<std::sync::mpsc::Sender<PlayCommand>> = OnceLock::new();

    fn get_audio_channel() -> &'static std::sync::mpsc::Sender<PlayCommand> {
        AUDIO_TX.get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                audio_worker_thread(rx);
            });
            tx
        })
    }

    struct PannedSource<I> {
        inner: I,
        left_gain: f32,
        right_gain: f32,
        next_right: Option<f32>,
    }

    impl<I> Iterator for PannedSource<I>
    where
        I: Source<Item = f32>,
    {
        type Item = f32;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if let Some(right) = self.next_right.take() {
                return Some(right);
            }
            if let Some(mono_sample) = self.inner.next() {
                self.next_right = Some(mono_sample * self.right_gain);
                Some(mono_sample * self.left_gain)
            } else {
                None
            }
        }
    }

    impl<I> Source for PannedSource<I>
    where
        I: Source<Item = f32>,
    {
        #[inline]
        fn current_span_len(&self) -> Option<usize> {
            self.inner.current_span_len().map(|l| l * 2)
        }

        #[inline]
        fn channels(&self) -> NonZero<u16> {
            NonZero::new(2).unwrap()
        }

        #[inline]
        fn sample_rate(&self) -> NonZero<u32> {
            self.inner.sample_rate()
        }

        #[inline]
        fn total_duration(&self) -> Option<Duration> {
            self.inner.total_duration()
        }

        #[inline]
        fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
            self.next_right = None;
            self.inner.try_seek(pos)
        }
    }

    struct SimpleRng {
        state: u32,
    }

    impl SimpleRng {
        fn new(seed: u32) -> Self {
            Self {
                state: if seed == 0 { 0x12345678 } else { seed },
            }
        }

        fn next_u32(&mut self) -> u32 {
            let mut x = self.state;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.state = x;
            x
        }

        fn next_f32(&mut self) -> f32 {
            (self.next_u32() & 0xFFFFFF) as f32 / 16777216.0
        }

        fn range(&mut self, min: f32, max: f32) -> f32 {
            min + self.next_f32() * (max - min)
        }
    }

    struct ArpeggioSource {
        sample_idx: u64,
        note_freqs: [f32; 4],
        note_durations: [u32; 4],
        duty: f32,
        decay_rate: f32,
        amplitude: f32,
        total_samples: u64,
    }

    impl ArpeggioSource {
        fn new(
            note_freqs: [f32; 4],
            note_durations: [u32; 4],
            duty: f32,
            decay_rate: f32,
            amplitude: f32,
        ) -> Self {
            let total_samples = note_durations.iter().map(|&d| d as u64).sum();
            Self {
                sample_idx: 0,
                note_freqs,
                note_durations,
                duty,
                decay_rate,
                amplitude,
                total_samples,
            }
        }
    }

    impl Iterator for ArpeggioSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            if self.sample_idx >= self.total_samples {
                return None;
            }

            let mut accumulated_samples = 0;
            let mut note_idx = 0;
            for i in 0..4 {
                let next_accum = accumulated_samples + self.note_durations[i] as u64;
                if self.sample_idx < next_accum {
                    note_idx = i;
                    break;
                }
                accumulated_samples = next_accum;
            }

            let note_sample_idx = self.sample_idx - accumulated_samples;
            let note_t = note_sample_idx as f32 / SAMPLE_RATE as f32;
            let freq = self.note_freqs[note_idx];

            self.sample_idx += 1;
            if freq < 1.0 {
                return Some(0.0);
            }

            let period = 1.0 / freq;
            let phase = (note_t % period) / period;
            let val = if phase < self.duty { 1.0 } else { -1.0 };

            let envelope = (-self.decay_rate * note_t).exp();

            let note_dur_secs = self.note_durations[note_idx] as f32 / SAMPLE_RATE as f32;
            let fade_start = note_dur_secs - 0.015;
            let final_fade = if note_t > fade_start {
                ((note_dur_secs - note_t) / 0.015).clamp(0.0, 1.0)
            } else {
                1.0
            };

            Some(val * envelope * final_fade * self.amplitude)
        }
    }

    impl Source for ArpeggioSource {
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
                self.total_samples as f32 / SAMPLE_RATE as f32,
            ))
        }
    }

    fn build_death_sound(
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
        let duty = 0.08;
        let amplitude = 0.12 * rng.range(0.9, 1.1);
        session.last_degree = base_degree;
        session.phrase_step = session.phrase_step.wrapping_add(1);
        ArpeggioSource::new(note_freqs, note_durations, duty, decay_rate, amplitude)
    }

    struct PulseSource {
        sample_idx: u64,
        freq: f32,
        duty: f32,
        decay_rate: f32,
        duration_samples: u64,
        amplitude: f32,
    }

    impl PulseSource {
        fn new(freq: f32, duty: f32, duration_secs: f32, decay_rate: f32, amplitude: f32) -> Self {
            Self {
                sample_idx: 0,
                freq: freq.max(20.0),
                duty: duty.clamp(0.05, 0.95),
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
            let period = 1.0 / self.freq;
            let phase = (t % period) / period;
            let val = if phase < self.duty { 1.0 } else { -1.0 };
            let envelope = (-self.decay_rate * t).exp();
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

    const PENTATONIC_SCALE: [f32; 15] = [
        110.00, 130.81, 146.83, 164.81, 196.00, 220.00, 261.63, 293.66, 329.63, 392.00, 440.00,
        523.25, 587.33, 659.25, 783.99,
    ];

    fn tile_hash(wx: f32, wy: f32) -> u32 {
        let ix = wx.floor() as u32;
        let iy = wy.floor() as u32;
        ix.wrapping_mul(73856093) ^ iy.wrapping_mul(19349663)
    }

    fn scale_index(octave: u8, degree: u8) -> usize {
        let o = (octave as usize).saturating_sub(2);
        (o * 5 + degree as usize).min(PENTATONIC_SCALE.len() - 1)
    }

    fn freq_at(octave: u8, degree: u8) -> f32 {
        PENTATONIC_SCALE[scale_index(octave, degree.min(4))]
    }

    fn degrees_to_freqs(octave: u8, degrees: &[u8; 4]) -> [f32; 4] {
        [
            freq_at(octave, degrees[0]),
            freq_at(octave, degrees[1]),
            freq_at(octave, degrees[2]),
            freq_at(octave, degrees[3]),
        ]
    }

    fn pick_base_degree(session: &MusicSession, wx: f32, wy: f32, rng: &mut SimpleRng) -> u8 {
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

    fn note_dur_samples(secs: f32) -> u32 {
        (SAMPLE_RATE as f32 * secs) as u32
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

    fn ensure_audio_stream(state: &mut AudioWorkerState) -> bool {
        if state.stream.is_some() {
            return true;
        }
        if state
            .open_backoff_until
            .is_some_and(|until| Instant::now() < until)
        {
            return false;
        }
        match DeviceSinkBuilder::open_default_sink() {
            Ok(mut stream) => {
                stream.log_on_drop(false);
                state.stream = Some(stream);
                state.open_backoff_until = None;
                true
            }
            Err(e) => {
                log::warn!("Failed to open default audio device: {e:?}");
                state.open_backoff_until = Some(Instant::now() + OPEN_BACKOFF);
                false
            }
        }
    }

    fn play_panned_source(
        state: &mut AudioWorkerState,
        source: BoxedSource,
        left: f32,
        right: f32,
        priority: SoundPriority,
        duration: Duration,
    ) {
        prune_voices(&mut state.voices);
        let active = active_voice_count(&state.voices);
        if !should_play(priority, active) {
            return;
        }
        if !ensure_audio_stream(state) {
            return;
        }
        let Some(stream) = state.stream.as_ref() else {
            return;
        };
        let gain = priority_gain(priority, active);
        let panned = PannedSource {
            inner: source,
            left_gain: left * gain,
            right_gain: right * gain,
            next_right: None,
        };
        state.voices.push(VoiceSlot {
            ends_at: Instant::now() + duration,
        });
        let player = Player::connect_new(stream.mixer());
        player.append(panned);
        player.detach();
    }

    fn audio_worker_thread(rx: std::sync::mpsc::Receiver<PlayCommand>) {
        let mut state = AudioWorkerState {
            stream: None,
            open_backoff_until: None,
            voices: Vec::new(),
        };

        while let Ok(cmd) = rx.recv() {
            play_panned_source(
                &mut state,
                cmd.source,
                cmd.left,
                cmd.right,
                cmd.priority,
                cmd.duration,
            );
        }
    }

    fn spatial_gains(spatial: super::SpatialSoundParams) -> (f32, f32, f32) {
        let super::SpatialSoundParams {
            wx,
            wy,
            camera_x,
            camera_y,
            camera_zoom,
            screen_w,
            screen_h,
        } = spatial;
        const ZOOM_FLOOR: f32 = 1.0;
        const ZOOM_FULL: f32 = 10.0;
        const ZOOM_MIN_GAIN: f32 = 0.0;

        let screen_x = camera_x + wx * camera_zoom;

        let p = (screen_x / screen_w.max(1.0)).clamp(0.0, 1.0);
        let pan = 0.15 + p * 0.70;
        let mut left = (1.0 - pan).sqrt();
        let mut right = pan.sqrt();

        let zoom = camera_zoom.max(0.001);
        let world_center_x = (screen_w / 2.0 - camera_x) / zoom;
        let world_center_y = (screen_h / 2.0 - camera_y) / zoom;
        let dx_world = wx - world_center_x;
        let dy_world = wy - world_center_y;
        let distance_tiles = (dx_world * dx_world + dy_world * dy_world).sqrt();
        let half_w = screen_w / (2.0 * zoom);
        let half_h = screen_h / (2.0 * zoom);
        let max_dist = (half_w * half_w + half_h * half_h).sqrt() * 1.5;
        let distance_factor = (1.0 - distance_tiles / max_dist.max(1.0)).clamp(0.0, 1.0);
        let distance_attenuation = distance_factor.sqrt();

        let zoom_attenuation = if camera_zoom >= ZOOM_FULL {
            1.0
        } else if camera_zoom <= ZOOM_FLOOR {
            ZOOM_MIN_GAIN
        } else {
            ZOOM_MIN_GAIN
                + (1.0 - ZOOM_MIN_GAIN) * (camera_zoom - ZOOM_FLOOR) / (ZOOM_FULL - ZOOM_FLOOR)
        };

        let total_volume = distance_attenuation * zoom_attenuation;
        left *= total_volume;
        right *= total_volume;

        (left, right, total_volume)
    }

    fn queue_spatial<S>(source: S, spatial: super::SpatialSoundParams, priority: SoundPriority)
    where
        S: Source<Item = f32> + Send + 'static,
    {
        let (left, right, total_volume) = spatial_gains(spatial);

        if total_volume > 0.01 {
            let duration = source_duration(&source);
            let _ = get_audio_channel().send(PlayCommand {
                source: BoxedSource(Box::new(source)),
                left,
                right,
                priority,
                duration,
            });
        }
    }

    pub fn play_spatial<S>(source: S, spatial: super::SpatialSoundParams)
    where
        S: Source<Item = f32> + Send + 'static,
    {
        queue_spatial(source, spatial, SoundPriority::Normal);
    }

    pub fn play_death_sound(
        player_type: PlayerSoundType,
        seed: u32,
        spatial: super::SpatialSoundParams,
    ) {
        let super::SpatialSoundParams { wx, wy, .. } = spatial;
        let source = {
            let mut session = music_session().lock().unwrap_or_else(|e| e.into_inner());
            build_death_sound(&mut session, player_type, seed, wx, wy)
        };
        queue_spatial(source, spatial, SoundPriority::Foreground);
    }

    pub fn play_deploy_sound(spatial: super::SpatialSoundParams) {
        let cursor = Cursor::new(DEPLOY_WAV);
        if let Ok(source) = rodio::Decoder::new(cursor) {
            queue_spatial(source.amplify(0.17), spatial, SoundPriority::Normal);
        }
    }

    pub fn play_combat_sound(
        kind: CombatSoundKind,
        troops: f32,
        seed: u32,
        spatial: super::SpatialSoundParams,
    ) {
        let super::SpatialSoundParams { wx, wy, .. } = spatial;
        let source = {
            let mut session = music_session().lock().unwrap_or_else(|e| e.into_inner());
            build_procedural_sound(&mut session, kind, troops, seed, wx, wy)
        };
        queue_spatial(source, spatial, SoundPriority::Background);
    }

    pub fn play_building_placement_sound(
        kind: BuildingSoundKind,
        spatial: super::SpatialSoundParams,
    ) {
        queue_spatial(
            BuildingPlacementSource::new(kind),
            spatial,
            SoundPriority::Normal,
        );
    }

    pub fn play_building_completed_sound(
        kind: BuildingSoundKind,
        spatial: super::SpatialSoundParams,
    ) {
        let _ = kind;
        queue_spatial(
            BuildingCompletionSource::new(),
            spatial,
            SoundPriority::Foreground,
        );
    }

    pub fn play_nuke_launch_sound(spatial: super::SpatialSoundParams) {
        queue_spatial(NukeLaunchSource::new(), spatial, SoundPriority::Foreground);
    }

    pub fn play_nuke_impact_sound(level: u8, spatial: super::SpatialSoundParams) {
        queue_spatial(
            NukeImpactSource::new(level),
            spatial,
            SoundPriority::Foreground,
        );
    }

    pub fn play_bunker_defense_sound(seed: u32, spatial: super::SpatialSoundParams) {
        queue_spatial(
            BunkerDefenseSource::new(seed),
            spatial,
            SoundPriority::Normal,
        );
    }

    pub fn play_ui<S>(source: S)
    where
        S: Source<Item = f32> + Send + 'static,
    {
        let duration = source_duration(&source);
        let _ = get_audio_channel().send(PlayCommand {
            source: BoxedSource(Box::new(source)),
            left: 1.0,
            right: 1.0,
            priority: SoundPriority::Foreground,
            duration,
        });
    }

    fn build_victory_sound() -> ArpeggioSource {
        let note_freqs = [523.25, 659.25, 783.99, 1046.50];
        let note_dur_samples = (SAMPLE_RATE as f32 * 0.15) as u32;
        let note_durations = [note_dur_samples; 4];
        ArpeggioSource::new(note_freqs, note_durations, 0.50, 6.0, 0.15)
    }

    fn build_defeat_sound() -> ArpeggioSource {
        let note_freqs = [261.63, 196.00, 164.81, 130.81];
        let note_dur_samples = (SAMPLE_RATE as f32 * 0.25) as u32;
        let note_durations = [note_dur_samples; 4];
        ArpeggioSource::new(note_freqs, note_durations, 0.125, 3.5, 0.15)
    }

    pub fn play_victory_sound() {
        play_ui(build_victory_sound());
    }

    pub fn play_defeat_sound() {
        play_ui(build_defeat_sound());
    }
}
