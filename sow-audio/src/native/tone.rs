//! Warm harmonic oscillator and envelope helpers for mobile-RTS SFX.

pub(super) fn warm_harmonic(angle: f32) -> f32 {
    angle.sin() * 0.70 + (2.0 * angle).sin() * 0.25 + (3.0 * angle).sin() * 0.05
}

pub(super) fn warm_at(freq: f32, t: f32) -> f32 {
    let angle = 2.0 * std::f32::consts::PI * freq.max(20.0) * t;
    warm_harmonic(angle)
}

pub(super) fn exp_decay(t: f32, rate: f32) -> f32 {
    (-rate * t).exp()
}

pub(super) fn tail_fade(t: f32, duration: f32, fade_secs: f32) -> f32 {
    if fade_secs <= 0.0 {
        return 1.0;
    }
    let fade_start = duration - fade_secs;
    if t > fade_start {
        ((duration - t) / fade_secs).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

pub(super) fn soft_attack(t: f32, attack_secs: f32) -> f32 {
    if attack_secs <= 0.0 {
        return 1.0;
    }
    (t / attack_secs).clamp(0.0, 1.0)
}

pub(super) fn note_envelope(
    note_t: f32,
    note_dur_secs: f32,
    decay_rate: f32,
    attack_secs: f32,
    tail_secs: f32,
) -> f32 {
    soft_attack(note_t, attack_secs)
        * exp_decay(note_t, decay_rate)
        * tail_fade(note_t, note_dur_secs, tail_secs)
}

pub(super) fn sweep_envelope(
    t: f32,
    duration: f32,
    decay_rate: f32,
    attack_secs: f32,
    tail_secs: f32,
) -> f32 {
    soft_attack(t, attack_secs)
        * exp_decay(t, decay_rate)
        * tail_fade(t, duration, tail_secs)
}
