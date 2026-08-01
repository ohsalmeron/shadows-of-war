use std::num::NonZero;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use rodio::source::Source;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

use super::tone::{note_envelope, warm_at};

pub(super) const SAMPLE_RATE: u32 = 22050;
pub(super) const OPEN_BACKOFF: Duration = Duration::from_secs(2);
pub(super) const MAX_VOICES: u8 = 3; // ponytail: reduced to 3 for stability and less clutter

pub(super) static MASTER_VOLUME: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(500); // ponytail: 50% default volume (halfway headroom up to 2x)

pub fn set_master_volume(volume: f32) {
    let vol_u32 = (volume * 1000.0).clamp(0.0, 1000.0) as u32;
    MASTER_VOLUME.store(vol_u32, std::sync::atomic::Ordering::Relaxed);
}

pub(super) static LAST_BUNKER_SOUND_MS: AtomicU64 = AtomicU64::new(0);
pub(super) static LAST_COMBAT_SOUND_MS: AtomicU64 = AtomicU64::new(0);

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SoundPriority {
    Background,
    Normal,
    Foreground,
}

pub(super) struct PlayCommand {
    source: BoxedSource,
    left: f32,
    right: f32,
    priority: SoundPriority,
    duration: Duration,
}

pub(super) struct VoiceSlot {
    ends_at: Instant,
}

pub(super) struct BoxedSource(Box<dyn Source<Item = f32> + Send>);

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

pub(super) struct AudioWorkerState {
    stream: Option<MixerDeviceSink>,
    open_backoff_until: Option<Instant>,
    voices: Vec<VoiceSlot>,
}

pub(super) fn prune_voices(voices: &mut Vec<VoiceSlot>) {
    let now = Instant::now();
    voices.retain(|v| v.ends_at > now);
}

pub(super) fn active_voice_count(voices: &[VoiceSlot]) -> u8 {
    voices.len().min(u8::MAX as usize) as u8
}

pub(super) fn should_play(priority: SoundPriority, active: u8) -> bool {
    match priority {
        SoundPriority::Background => active < MAX_VOICES,
        SoundPriority::Normal => active < MAX_VOICES + 1,
        SoundPriority::Foreground => true,
    }
}

pub(super) fn priority_gain(priority: SoundPriority, active: u8) -> f32 {
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

pub(super) fn source_duration<S: Source<Item = f32>>(source: &S) -> Duration {
    source
        .total_duration()
        .unwrap_or(Duration::from_millis(150))
}

static AUDIO_TX: OnceLock<std::sync::mpsc::Sender<PlayCommand>> = OnceLock::new();

pub(super) fn get_audio_channel() -> &'static std::sync::mpsc::Sender<PlayCommand> {
    AUDIO_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            audio_worker_thread(rx);
        });
        tx
    })
}

pub(super) struct PannedSource<I> {
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

pub(super) struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    pub(super) fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x12345678 } else { seed },
        }
    }

    pub(super) fn next_u32(&mut self) -> u32 {
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

    pub(super) fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

pub(super) struct ArpeggioSource {
    sample_idx: u64,
    note_freqs: [f32; 4],
    note_durations: [u32; 4],
    decay_rate: f32,
    amplitude: f32,
    total_samples: u64,
}

impl ArpeggioSource {
    pub(super) fn new(
        note_freqs: [f32; 4],
        note_durations: [u32; 4],
        decay_rate: f32,
        amplitude: f32,
    ) -> Self {
        let total_samples = note_durations.iter().map(|&d| d as u64).sum();
        Self {
            sample_idx: 0,
            note_freqs,
            note_durations,
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

        let val = warm_at(freq, note_t);

        let note_dur_secs = self.note_durations[note_idx] as f32 / SAMPLE_RATE as f32;
        let envelope = note_envelope(note_t, note_dur_secs, self.decay_rate, 0.005, 0.015);

        Some(val * envelope * self.amplitude)
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

pub(super) fn ensure_audio_stream(state: &mut AudioWorkerState) -> bool {
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

pub(super) fn play_panned_source(
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
    let master = MASTER_VOLUME.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
    let gain = priority_gain(priority, active) * master;
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

pub(super) fn audio_worker_thread(rx: std::sync::mpsc::Receiver<PlayCommand>) {
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

pub(super) fn spatial_gains(spatial: crate::SpatialSoundParams) -> (f32, f32, f32) {
    let crate::SpatialSoundParams {
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

pub(super) fn queue_spatial<S>(
    source: S,
    spatial: crate::SpatialSoundParams,
    priority: SoundPriority,
) where
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

pub fn play_spatial<S>(source: S, spatial: crate::SpatialSoundParams)
where
    S: Source<Item = f32> + Send + 'static,
{
    queue_spatial(source, spatial, SoundPriority::Normal);
}
pub fn play_ui<S>(source: S)
where
    S: Source<Item = f32> + Send + 'static,
{
    let duration = source_duration(&source);
    let master = MASTER_VOLUME.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
    let _ = get_audio_channel().send(PlayCommand {
        source: BoxedSource(Box::new(source)),
        left: master,
        right: master,
        priority: SoundPriority::Foreground,
        duration,
    });
}
