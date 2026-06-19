#!/usr/bin/env python3
import argparse
import math
import struct
import wave
import os

# Mobile-RTS harmonic synthesizer CLI tool.
# Synthesizes 16-bit 22050Hz Mono PCM WAV files programmatically.
# Matches the warm harmonic oscillator in sow-audio/src/native/tone.rs.

SAMPLE_RATE = 22050
DEFAULT_ATTACK = 0.005


def warm_harmonic(t, freq):
    """Layered sine harmonics (fundamental + 2nd + 3rd) matching Rust tone.rs."""
    angle = 2.0 * math.pi * freq * t
    return (
        math.sin(angle) * 0.70
        + math.sin(2.0 * angle) * 0.25
        + math.sin(3.0 * angle) * 0.05
    )


def soft_attack(t, attack_secs=DEFAULT_ATTACK):
    if attack_secs <= 0.0:
        return 1.0
    return min(1.0, t / attack_secs)


def tail_fade(t, duration, fade_secs):
    if fade_secs <= 0.0:
        return 1.0
    fade_start = duration - fade_secs
    if t > fade_start:
        return max(0.0, (duration - t) / fade_secs)
    return 1.0


def note_envelope(t, duration, decay_rate, attack_secs=DEFAULT_ATTACK, tail_secs=0.015):
    return soft_attack(t, attack_secs) * math.exp(-decay_rate * t) * tail_fade(t, duration, tail_secs)


def sweep_envelope(t, duration, decay_rate, attack_secs=DEFAULT_ATTACK, tail_secs=0.03):
    return note_envelope(t, duration, decay_rate, attack_secs, tail_secs)


def synth_note(freq, duration, decay_rate=6.0, attack_secs=DEFAULT_ATTACK, tail_secs=0.015):
    """Synthesizes a single warm harmonic note."""
    samples = []
    num_samples = int(SAMPLE_RATE * duration)
    for i in range(num_samples):
        t = float(i) / SAMPLE_RATE
        val = warm_harmonic(t, freq)
        envelope = note_envelope(t, duration, decay_rate, attack_secs, tail_secs)
        samples.append(val * envelope)
    return samples


def synth_sweep(start_freq, end_freq, duration, decay_rate=10.0, attack_secs=0.006, tail_secs=0.025):
    """Warm harmonic frequency sweep."""
    samples = []
    num_samples = int(SAMPLE_RATE * duration)
    for i in range(num_samples):
        t = float(i) / SAMPLE_RATE
        progress = float(i) / max(1, num_samples - 1)
        freq = start_freq + (end_freq - start_freq) * progress
        val = warm_harmonic(t, freq)
        envelope = sweep_envelope(t, duration, decay_rate, attack_secs, tail_secs)
        samples.append(val * envelope)
    return samples


def preset_death():
    """Player defeated arpeggio (C6 -> G5 -> E5 -> C5)"""
    arpeggio = [1046.50, 783.99, 659.25, 523.25]
    note_duration = 0.15
    all_samples = []
    for freq in arpeggio:
        all_samples.extend(synth_note(freq, note_duration, decay_rate=6.0))
    return all_samples


def preset_upgrade():
    """Building completion ascending arpeggio (C5 -> E5 -> G5 -> C6)"""
    arpeggio = [523.25, 659.25, 783.99, 1046.50]
    note_duration = 0.07
    all_samples = []
    for freq in arpeggio:
        all_samples.extend(synth_note(freq, note_duration, decay_rate=8.0, tail_secs=0.02))
    return all_samples


def preset_click():
    """Short warm UI click"""
    return synth_note(440.0, 0.05, decay_rate=18.0, attack_secs=0.003, tail_secs=0.01)


def preset_deploy():
    """Warm upward sweep for spawn/deployment (200 Hz -> 450 Hz)"""
    return synth_sweep(200.0, 450.0, 0.12, decay_rate=10.0)


def preset_conquer():
    """Victory fanfare (C5 -> E5 -> G5 -> C6)"""
    arpeggio = [523.25, 659.25, 783.99, 1046.50]
    note_duration = 0.15
    all_samples = []
    for freq in arpeggio:
        all_samples.extend(synth_note(freq, note_duration, decay_rate=5.5))
    return all_samples


def preset_nuke():
    """Warm low rumble + mid bloom explosion"""
    duration = 1.2
    num_samples = int(SAMPLE_RATE * duration)
    samples = []
    for i in range(num_samples):
        t = float(i) / SAMPLE_RATE

        low_freq = 80.0 * math.exp(-3.5 * t) + 35.0
        mid_freq = 180.0 * math.exp(-5.0 * t) + 60.0
        low = warm_harmonic(t, low_freq)
        mid = warm_harmonic(t, mid_freq) * 0.45
        val = low * 0.65 + mid * 0.35

        envelope = math.exp(-2.2 * t)
        if t > duration - 0.2:
            linear_fade = (duration - t) / 0.2
            envelope *= max(0.0, linear_fade)

        samples.append(val * envelope)
    return samples


def save_wav(path, samples, amplitude=0.18):
    """Saves synthesized floats as 16-bit PCM Mono WAV"""
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with wave.open(path, "w") as f:
        f.setnchannels(1)
        f.setsampwidth(2)
        f.setframerate(SAMPLE_RATE)

        for val in samples:
            clamped = max(-1.0, min(1.0, val * amplitude))
            pcm_val = int(clamped * 32767.0)
            f.writeframesraw(struct.pack("<h", pcm_val))

    print(f"Generated {path} ({len(samples)} samples, {len(samples)/SAMPLE_RATE:.2f}s)")


def main():
    parser = argparse.ArgumentParser(description="Mobile-RTS Harmonic Sound Synthesizer")
    parser.add_argument(
        "--preset",
        choices=["death", "upgrade", "click", "nuke", "conquer", "deploy"],
        required=True,
        help="Sound effect preset to generate",
    )
    parser.add_argument(
        "-o", "--output",
        required=True,
        help="Output file path (e.g. assets/static/ui/death.wav)",
    )
    parser.add_argument(
        "--volume",
        type=float,
        default=1.0,
        help="Relative volume factor (default: 1.0)",
    )
    args = parser.parse_args()

    presets = {
        "death": preset_death,
        "upgrade": preset_upgrade,
        "click": preset_click,
        "nuke": preset_nuke,
        "conquer": preset_conquer,
        "deploy": preset_deploy,
    }

    print(f"Synthesizing '{args.preset}' preset...")
    samples = presets[args.preset]()

    save_wav(args.output, samples, amplitude=0.18 * args.volume)


if __name__ == "__main__":
    main()
