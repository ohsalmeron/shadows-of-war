#!/usr/bin/env python3
import argparse
import math
import struct
import wave
import os

# NES-style retro synthesizer CLI tool.
# Synthesizes 16-bit 22050Hz Mono PCM WAV files programmatically.

SAMPLE_RATE = 22050

def pulse_wave(t, freq, duty_cycle=0.25):
    """
    Returns pulse wave sample value (-1.0 to 1.0)
    duty_cycle=0.125: 12.5% duty cycle (buzzy)
    duty_cycle=0.25:  25% duty cycle (retro bright)
    duty_cycle=0.50:  50% duty cycle (standard square)
    """
    period = 1.0 / freq
    phase = (t % period) / period
    return 1.0 if phase < duty_cycle else -1.0

def triangle_wave(t, freq):
    """Returns triangle wave sample value (-1.0 to 1.0) (softer flute-like sound)"""
    period = 1.0 / freq
    phase = (t % period) / period
    return 2.0 * abs(2.0 * phase - 1.0) - 1.0

def sawtooth_wave(t, freq):
    """Returns sawtooth wave sample value (-1.0 to 1.0) (harsh brassy synth)"""
    period = 1.0 / freq
    phase = (t % period) / period
    return 2.0 * phase - 1.0

def sine_wave(t, freq):
    """Returns classic sine wave sample value (-1.0 to 1.0)"""
    return math.sin(2.0 * math.pi * freq * t)

def synth_note(freq, duration, wave_type="pulse", duty=0.25, decay_rate=6.0):
    """Synthesizes a single note with exponential decay envelope."""
    samples = []
    num_samples = int(SAMPLE_RATE * duration)
    for i in range(num_samples):
        t = float(i) / SAMPLE_RATE
        
        # Select oscillator
        if wave_type == "pulse":
            val = pulse_wave(t, freq, duty)
        elif wave_type == "triangle":
            val = triangle_wave(t, freq)
        elif wave_type == "sawtooth":
            val = sawtooth_wave(t, freq)
        elif wave_type == "sine":
            val = sine_wave(t, freq)
        else:
            val = pulse_wave(t, freq, 0.50) # default to square
            
        # Envelope generator (exponential decay + final linear fadeout)
        envelope = math.exp(-decay_rate * t)
        fadeout_start = duration - 0.05
        if t > fadeout_start:
            # Linear fade in the last 50ms
            linear_fade = (duration - t) / 0.05
            envelope *= max(0.0, linear_fade)
            
        samples.append(val * envelope)
    return samples

def preset_death():
    """Authentic NES/arcade player defeated arpeggio (C6 -> G5 -> E5 -> C5)"""
    arpeggio = [1046.50, 783.99, 659.25, 523.25]
    note_duration = 0.15
    all_samples = []
    for freq in arpeggio:
        all_samples.extend(synth_note(freq, note_duration, "pulse", duty=0.25, decay_rate=6.0))
    return all_samples

def preset_upgrade():
    """Retro 8-bit level completion or building upgrade sound (ascending sweep)"""
    # Quick ascending arpeggio (C5 -> E5 -> G5 -> C6 -> E6 -> G6)
    arpeggio = [523.25, 659.25, 783.99, 1046.50, 1318.51, 1567.98]
    note_duration = 0.08
    all_samples = []
    for freq in arpeggio:
        all_samples.extend(synth_note(freq, note_duration, "pulse", duty=0.50, decay_rate=5.0))
    return all_samples

def preset_click():
    """Snappy, dry retro menu UI click"""
    # Extremely short high pitch pop
    return synth_note(880.0, 0.04, "pulse", duty=0.125, decay_rate=30.0)

def preset_deploy():
    """Snappy dual-tone ascending arpeggio for spawn/deployment placement (G5 -> C6)"""
    notes = [
        (783.99, 0.06, 0.125, 12.0),
        (1046.50, 0.10, 0.25, 8.0),
    ]
    all_samples = []
    for freq, dur, duty, decay in notes:
        all_samples.extend(synth_note(freq, dur, "pulse", duty=duty, decay_rate=decay))
    return all_samples

def preset_conquer():
    """Satisfying triumph sound for reclaiming territory or conquering a player"""
    # Fanfare: C5 -> G5 -> C6 (long)
    fanfare = [
        (523.25, 0.10, 4.0),
        (783.99, 0.10, 4.0),
        (1046.50, 0.40, 3.0),
    ]
    all_samples = []
    for freq, dur, decay in fanfare:
        all_samples.extend(synth_note(freq, dur, "pulse", duty=0.25, decay_rate=decay))
    return all_samples

def preset_nuke():
    """Thick, dirty explosion/rumble with low-end sawtooth and noise-like frequency modulation"""
    duration = 1.2
    num_samples = int(SAMPLE_RATE * duration)
    samples = []
    for i in range(num_samples):
        t = float(i) / SAMPLE_RATE
        
        # Sweep frequency downwards aggressively from 120Hz to 20Hz
        freq = 120.0 * math.exp(-4.0 * t) + 20.0
        
        # Add frequency modulation to sound "dirty/gritty" like an explosion
        fm = math.sin(2.0 * math.pi * 150.0 * t) * 8.0
        
        # Sawtooth wave + low-frequency triangle wave
        val = sawtooth_wave(t, freq + fm) * 0.7 + triangle_wave(t, freq * 0.5) * 0.3
        
        # Long rumble decay
        envelope = math.exp(-2.5 * t)
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
            # Apply scaling amplitude and clamp to 16-bit range
            clamped = max(-1.0, min(1.0, val * amplitude))
            pcm_val = int(clamped * 32767.0)
            f.writeframesraw(struct.pack("<h", pcm_val))
            
    print(f"✅ Generated {path} ({len(samples)} samples, {len(samples)/SAMPLE_RATE:.2f}s)")

def main():
    parser = argparse.ArgumentParser(description="NES-style Retro Sound Synthesizer")
    parser.add_argument(
        "--preset",
        choices=["death", "upgrade", "click", "nuke", "conquer", "deploy"],
        required=True,
        help="Sound effect preset to generate"
    )
    parser.add_argument(
        "-o", "--output",
        required=True,
        help="Output file path (e.g. assets/static/ui/death.wav)"
    )
    parser.add_argument(
        "--volume",
        type=float,
        default=1.0,
        help="Relative volume factor (default: 1.0)"
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
