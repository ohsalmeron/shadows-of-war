//! Simple synthesized retro audio effects for game events.
//! Completely programmatic and lightweight, avoiding assets/bloat.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use rodio::{DeviceSinkBuilder, MixerDeviceSink};
#[cfg(not(target_arch = "wasm32"))]
use rodio::mixer::Mixer;
#[cfg(not(target_arch = "wasm32"))]
use rodio::source::{SineWave, SawtoothWave, Source};

#[cfg(not(target_arch = "wasm32"))]
struct AudioState {
    _stream: MixerDeviceSink,
    mixer: Mixer,
}

#[cfg(not(target_arch = "wasm32"))]
static AUDIO_STATE: OnceLock<Option<AudioState>> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn get_audio_mixer() -> Option<&'static Mixer> {
    AUDIO_STATE.get_or_init(|| {
        if let Ok(stream) = DeviceSinkBuilder::open_default_sink() {
            let mixer = stream.mixer().clone();
            Some(AudioState {
                _stream: stream,
                mixer,
            })
        } else {
            None
        }
    }).as_ref().map(|s| &s.mixer)
}

/// Plays a retro descending synthesizer sound when any player is eliminated.
/// Plays via native rodio on desktop, and is a no-op on wasm32 targets to preserve a lean binary size.
pub fn play_death_sound() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(mixer) = get_audio_mixer() {
            // Note 1: High Sine
            let wave1 = SineWave::new(220.0)
                .take_duration(Duration::from_millis(400))
                .fade_out(Duration::from_millis(100))
                .amplify(0.10);

            // Note 2: Mid-High Sine delayed
            let wave2 = SineWave::new(165.0)
                .take_duration(Duration::from_millis(400))
                .delay(Duration::from_millis(100))
                .fade_out(Duration::from_millis(100))
                .amplify(0.10);

            // Note 3: Low-Mid Sawtooth delayed
            let wave3 = SawtoothWave::new(110.0)
                .take_duration(Duration::from_millis(400))
                .delay(Duration::from_millis(200))
                .fade_out(Duration::from_millis(100))
                .amplify(0.04);

            // Note 4: Deep Sub Sawtooth delayed
            let wave4 = SawtoothWave::new(55.0)
                .take_duration(Duration::from_millis(400))
                .delay(Duration::from_millis(300))
                .fade_out(Duration::from_millis(150))
                .amplify(0.04);

            mixer.add(wave1);
            mixer.add(wave2);
            mixer.add(wave3);
            mixer.add(wave4);
        }
    }
}
