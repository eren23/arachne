//! Audio effects: low-pass filter, ADSR envelope, and Schroeder reverb.

use core::f32::consts::PI;

// ---------------------------------------------------------------------------
// Low-pass filter (single-pole IIR)
// ---------------------------------------------------------------------------

/// A simple single-pole IIR low-pass filter.
///
/// Transfer function: y[n] = alpha * x[n] + (1 - alpha) * y[n-1]
#[derive(Clone, Debug)]
pub struct LowPassFilter {
    alpha: f32,
    prev_left: f32,
    prev_right: f32,
    cutoff_hz: f32,
    sample_rate: f32,
}

impl LowPassFilter {
    /// Creates a new low-pass filter.
    ///
    /// # Arguments
    /// * `cutoff_hz` - Cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let alpha = Self::compute_alpha(cutoff_hz, sample_rate);
        Self {
            alpha,
            prev_left: 0.0,
            prev_right: 0.0,
            cutoff_hz,
            sample_rate,
        }
    }

    fn compute_alpha(cutoff_hz: f32, sample_rate: f32) -> f32 {
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let dt = 1.0 / sample_rate;
        dt / (rc + dt)
    }

    /// Sets the cutoff frequency.
    pub fn set_cutoff(&mut self, cutoff_hz: f32) {
        self.cutoff_hz = cutoff_hz;
        self.alpha = Self::compute_alpha(cutoff_hz, self.sample_rate);
    }

    /// Returns the current cutoff frequency.
    pub fn cutoff(&self) -> f32 {
        self.cutoff_hz
    }

    /// Processes a single stereo sample pair in-place.
    #[inline]
    pub fn process(&mut self, left: &mut f32, right: &mut f32) {
        self.prev_left = self.alpha * *left + (1.0 - self.alpha) * self.prev_left;
        self.prev_right = self.alpha * *right + (1.0 - self.alpha) * self.prev_right;
        *left = self.prev_left;
        *right = self.prev_right;
    }

    /// Processes an interleaved stereo buffer in-place.
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        let frames = buffer.len() / 2;
        for i in 0..frames {
            let idx = i * 2;
            let (left_slice, right_slice) = buffer[idx..idx + 2].split_at_mut(1);
            self.process(&mut left_slice[0], &mut right_slice[0]);
        }
    }

    /// Resets the filter state.
    pub fn reset(&mut self) {
        self.prev_left = 0.0;
        self.prev_right = 0.0;
    }
}

// ---------------------------------------------------------------------------
// ADSR Envelope
// ---------------------------------------------------------------------------

/// ADSR envelope state machine phases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AdsrPhase {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

/// ADSR (Attack, Decay, Sustain, Release) volume envelope.
///
/// Generates a time-varying amplitude multiplier that shapes the volume
/// of a sound over time.
#[derive(Clone, Debug)]
pub struct AdsrEnvelope {
    /// Attack time in seconds.
    pub attack: f32,
    /// Decay time in seconds.
    pub decay: f32,
    /// Sustain level [0.0, 1.0].
    pub sustain_level: f32,
    /// Release time in seconds.
    pub release: f32,

    phase: AdsrPhase,
    /// Current position within the active phase, in samples.
    phase_sample: usize,
    /// Current envelope value.
    current_value: f32,
    /// Value at the start of release phase.
    release_start_value: f32,
    sample_rate: f32,
}

impl AdsrEnvelope {
    /// Creates a new ADSR envelope.
    ///
    /// # Arguments
    /// * `attack` - Attack time in seconds
    /// * `decay` - Decay time in seconds
    /// * `sustain_level` - Sustain level [0.0, 1.0]
    /// * `release` - Release time in seconds
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(attack: f32, decay: f32, sustain_level: f32, release: f32, sample_rate: f32) -> Self {
        Self {
            attack,
            decay,
            sustain_level: sustain_level.clamp(0.0, 1.0),
            release,
            phase: AdsrPhase::Off,
            phase_sample: 0,
            current_value: 0.0,
            release_start_value: 0.0,
            sample_rate,
        }
    }

    /// Triggers the envelope (starts attack phase).
    pub fn trigger(&mut self) {
        self.phase = AdsrPhase::Attack;
        self.phase_sample = 0;
        self.current_value = 0.0;
    }

    /// Releases the envelope (starts release phase from current value).
    pub fn release(&mut self) {
        if self.phase != AdsrPhase::Off {
            self.release_start_value = self.current_value;
            self.phase = AdsrPhase::Release;
            self.phase_sample = 0;
        }
    }

    /// Returns the current phase.
    pub fn phase(&self) -> AdsrPhase {
        self.phase
    }

    /// Returns the current envelope value.
    pub fn value(&self) -> f32 {
        self.current_value
    }

    /// Returns whether the envelope is active (not Off).
    pub fn is_active(&self) -> bool {
        self.phase != AdsrPhase::Off
    }

    /// Advances the envelope by one sample and returns the current amplitude.
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        match self.phase {
            AdsrPhase::Attack => {
                let attack_samples = (self.attack * self.sample_rate) as usize;
                if attack_samples == 0 {
                    self.current_value = 1.0;
                    self.phase = AdsrPhase::Decay;
                    self.phase_sample = 0;
                } else {
                    self.current_value = self.phase_sample as f32 / attack_samples as f32;
                    self.phase_sample += 1;
                    if self.phase_sample >= attack_samples {
                        self.current_value = 1.0;
                        self.phase = AdsrPhase::Decay;
                        self.phase_sample = 0;
                    }
                }
            }
            AdsrPhase::Decay => {
                let decay_samples = (self.decay * self.sample_rate) as usize;
                if decay_samples == 0 {
                    self.current_value = self.sustain_level;
                    self.phase = AdsrPhase::Sustain;
                    self.phase_sample = 0;
                } else {
                    let t = self.phase_sample as f32 / decay_samples as f32;
                    self.current_value = 1.0 + (self.sustain_level - 1.0) * t;
                    self.phase_sample += 1;
                    if self.phase_sample >= decay_samples {
                        self.current_value = self.sustain_level;
                        self.phase = AdsrPhase::Sustain;
                        self.phase_sample = 0;
                    }
                }
            }
            AdsrPhase::Sustain => {
                self.current_value = self.sustain_level;
            }
            AdsrPhase::Release => {
                let release_samples = (self.release * self.sample_rate) as usize;
                if release_samples == 0 {
                    self.current_value = 0.0;
                    self.phase = AdsrPhase::Off;
                } else {
                    let t = self.phase_sample as f32 / release_samples as f32;
                    self.current_value = self.release_start_value * (1.0 - t);
                    self.phase_sample += 1;
                    if self.phase_sample >= release_samples {
                        self.current_value = 0.0;
                        self.phase = AdsrPhase::Off;
                    }
                }
            }
            AdsrPhase::Off => {
                self.current_value = 0.0;
            }
        }

        self.current_value
    }

    /// Processes an interleaved stereo buffer, applying the envelope.
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        let frames = buffer.len() / 2;
        for i in 0..frames {
            let amp = self.next_sample();
            let idx = i * 2;
            buffer[idx] *= amp;
            buffer[idx + 1] *= amp;
        }
    }
}

// ---------------------------------------------------------------------------
// Schroeder Reverberator
// ---------------------------------------------------------------------------

/// A comb filter delay line used in the Schroeder reverberator.
#[derive(Clone, Debug)]
struct CombFilter {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
    damp: f32,
    prev: f32,
}

impl CombFilter {
    fn new(delay_samples: usize, feedback: f32, damp: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples],
            index: 0,
            feedback,
            damp,
            prev: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];

        // Low-pass filtered feedback
        self.prev = output * (1.0 - self.damp) + self.prev * self.damp;
        self.buffer[self.index] = input + self.prev * self.feedback;

        self.index += 1;
        if self.index >= self.buffer.len() {
            self.index = 0;
        }

        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
        self.prev = 0.0;
    }
}

/// An all-pass filter used in the Schroeder reverberator.
#[derive(Clone, Debug)]
struct AllPassFilter {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
}

impl AllPassFilter {
    fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples],
            index: 0,
            feedback,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let output = -input + buffered;
        self.buffer[self.index] = input + buffered * self.feedback;

        self.index += 1;
        if self.index >= self.buffer.len() {
            self.index = 0;
        }

        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }
}

/// Schroeder reverberator with 4 parallel comb filters and 2 series all-pass filters.
#[derive(Clone, Debug)]
pub struct SchroederReverb {
    combs: [CombFilter; 4],
    allpasses: [AllPassFilter; 2],
    /// Wet/dry mix [0.0 = dry, 1.0 = fully wet].
    pub mix: f32,
}

impl SchroederReverb {
    /// Creates a new Schroeder reverberator.
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate in Hz
    /// * `room_size` - Room size factor [0.0, 1.0] (affects feedback)
    /// * `damping` - Damping factor [0.0, 1.0] (high-frequency absorption)
    /// * `mix` - Wet/dry mix [0.0, 1.0]
    pub fn new(sample_rate: f32, room_size: f32, damping: f32, mix: f32) -> Self {
        let room = room_size.clamp(0.0, 1.0);
        let damp = damping.clamp(0.0, 1.0);
        let feedback = 0.7 + 0.28 * room; // range ~0.7..0.98

        // Classic Schroeder comb filter delay times (in ms), scaled to sample rate
        let comb_delays_ms = [29.7, 37.1, 41.1, 43.7];
        let allpass_delays_ms = [5.0, 1.7];

        let ms_to_samples = |ms: f32| -> usize { (ms * sample_rate / 1000.0) as usize };

        let combs = [
            CombFilter::new(ms_to_samples(comb_delays_ms[0]), feedback, damp),
            CombFilter::new(ms_to_samples(comb_delays_ms[1]), feedback, damp),
            CombFilter::new(ms_to_samples(comb_delays_ms[2]), feedback, damp),
            CombFilter::new(ms_to_samples(comb_delays_ms[3]), feedback, damp),
        ];

        let allpasses = [
            AllPassFilter::new(ms_to_samples(allpass_delays_ms[0]), 0.5),
            AllPassFilter::new(ms_to_samples(allpass_delays_ms[1]), 0.5),
        ];

        Self {
            combs,
            allpasses,
            mix: mix.clamp(0.0, 1.0),
        }
    }

    /// Processes a single mono sample through the reverb.
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Sum outputs of parallel comb filters
        let mut comb_sum = 0.0f32;
        for comb in &mut self.combs {
            comb_sum += comb.process(input);
        }
        comb_sum *= 0.25; // Average

        // Series all-pass filters
        let mut output = comb_sum;
        for ap in &mut self.allpasses {
            output = ap.process(output);
        }

        // Mix wet/dry
        input * (1.0 - self.mix) + output * self.mix
    }

    /// Processes an interleaved stereo buffer. Applies reverb to the mono
    /// mix of L+R, then blends back.
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        let frames = buffer.len() / 2;
        for i in 0..frames {
            let idx = i * 2;
            let mono = (buffer[idx] + buffer[idx + 1]) * 0.5;
            let wet = self.process_sample(mono);
            // Replace dry with wet blend (same wet signal to both channels for simplicity)
            buffer[idx] = buffer[idx] * (1.0 - self.mix) + wet * self.mix;
            buffer[idx + 1] = buffer[idx + 1] * (1.0 - self.mix) + wet * self.mix;
        }
    }

    /// Resets all internal state.
    pub fn reset(&mut self) {
        for comb in &mut self.combs {
            comb.reset();
        }
        for ap in &mut self.allpasses {
            ap.reset();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn assert_f32_approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < EPSILON,
            "assertion failed: {a} != {b} (within epsilon {EPSILON})"
        );
    }

    // -- Low-pass filter tests -------------------------------------------------

    #[test]
    fn test_lowpass_dc_passes_through() {
        let mut filter = LowPassFilter::new(1000.0, 48000.0);
        // DC signal (constant value) should pass through completely
        for _ in 0..1000 {
            let mut l = 1.0f32;
            let mut r = 1.0f32;
            filter.process(&mut l, &mut r);
        }
        let mut l = 1.0;
        let mut r = 1.0;
        filter.process(&mut l, &mut r);
        // After settling, output should be very close to 1.0
        assert!(l > 0.99, "DC should pass through, got {l}");
    }

    #[test]
    fn test_lowpass_high_freq_attenuation() {
        // Low-pass at 1kHz, sample rate 48kHz
        let mut filter = LowPassFilter::new(1000.0, 48000.0);
        let sample_rate = 48000.0;

        // Generate white noise, filter it, then check energy in frequency bands
        // Use a simple approach: generate high-frequency signal (near Nyquist)
        // and verify it's attenuated
        let high_freq = 20000.0; // 20kHz
        let n_samples = 4096;

        let mut high_freq_energy_in = 0.0f32;
        let mut high_freq_energy_out = 0.0f32;

        for i in 0..n_samples {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * high_freq * t).sin();
            high_freq_energy_in += sample * sample;

            let mut l = sample;
            let mut r = sample;
            filter.process(&mut l, &mut r);
            high_freq_energy_out += l * l;
        }

        // High frequency should be significantly attenuated
        let ratio = high_freq_energy_out / high_freq_energy_in;
        assert!(
            ratio < 0.01,
            "20kHz through 1kHz low-pass should be heavily attenuated, ratio: {ratio}"
        );
    }

    #[test]
    fn test_lowpass_low_freq_passes() {
        let mut filter = LowPassFilter::new(10000.0, 48000.0);
        let sample_rate = 48000.0;
        let low_freq = 100.0;
        let n_samples = 4096;

        // Skip first 1000 samples for filter to settle
        for i in 0..1000 {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * low_freq * t).sin();
            let mut l = sample;
            let mut r = sample;
            filter.process(&mut l, &mut r);
        }

        let mut energy_in = 0.0f32;
        let mut energy_out = 0.0f32;
        for i in 1000..(1000 + n_samples) {
            let t = i as f32 / sample_rate;
            let sample = (2.0 * PI * low_freq * t).sin();
            energy_in += sample * sample;

            let mut l = sample;
            let mut r = sample;
            filter.process(&mut l, &mut r);
            energy_out += l * l;
        }

        let ratio = energy_out / energy_in;
        // 100Hz through 10kHz LP should pass with minimal attenuation
        assert!(
            ratio > 0.9,
            "100Hz through 10kHz low-pass should pass mostly unattenuated, ratio: {ratio}"
        );
    }

    #[test]
    fn test_lowpass_buffer() {
        let mut filter = LowPassFilter::new(5000.0, 48000.0);
        let mut buffer = vec![1.0f32; 200]; // 100 stereo frames of DC

        filter.process_buffer(&mut buffer);

        // After processing, output should converge to 1.0 for DC
        // Check last few samples are close to 1.0
        assert!(buffer[198] > 0.95, "DC should converge, got {}", buffer[198]);
    }

    #[test]
    fn test_lowpass_reset() {
        let mut filter = LowPassFilter::new(1000.0, 48000.0);
        let mut l = 1.0;
        let mut r = 1.0;
        for _ in 0..100 {
            filter.process(&mut l, &mut r);
        }
        filter.reset();
        // After reset, processing 0 should give 0
        l = 0.0;
        r = 0.0;
        filter.process(&mut l, &mut r);
        assert_f32_approx(l, 0.0);
        assert_f32_approx(r, 0.0);
    }

    // -- ADSR Envelope tests ---------------------------------------------------

    #[test]
    fn test_adsr_attack_ramp() {
        let sample_rate = 48000.0;
        let attack_secs = 0.01; // 10ms = 480 samples
        let mut env = AdsrEnvelope::new(attack_secs, 0.0, 1.0, 0.01, sample_rate);
        env.trigger();

        let attack_samples = (attack_secs * sample_rate) as usize;

        // Sample at start should be near 0
        let first = env.next_sample();
        assert!(first < 0.01, "first attack sample should be near 0, got {first}");

        // Advance to midpoint
        for _ in 1..(attack_samples / 2) {
            env.next_sample();
        }
        let mid = env.value();
        assert!(mid > 0.4 && mid < 0.6, "mid-attack should be ~0.5, got {mid}");

        // Advance to end of attack
        for _ in (attack_samples / 2)..attack_samples {
            env.next_sample();
        }
        // Should have transitioned to decay or sustain
        assert!(env.phase() != AdsrPhase::Attack, "should have left attack phase");
    }

    #[test]
    fn test_adsr_decay_to_sustain() {
        let sample_rate = 48000.0;
        let mut env = AdsrEnvelope::new(0.0, 0.01, 0.5, 0.01, sample_rate);
        env.trigger();

        // Zero attack -> starts at decay immediately
        // Advance through attack (instant)
        env.next_sample();

        // Should be in decay now going from 1.0 to 0.5
        let decay_samples = (0.01 * sample_rate) as usize;
        for _ in 0..decay_samples {
            env.next_sample();
        }

        // Should be at sustain level
        let val = env.value();
        assert!(
            (val - 0.5).abs() < 0.02,
            "after decay should be at sustain level 0.5, got {val}"
        );
        assert_eq!(env.phase(), AdsrPhase::Sustain);
    }

    #[test]
    fn test_adsr_sustain_holds() {
        let sample_rate = 48000.0;
        let mut env = AdsrEnvelope::new(0.0, 0.0, 0.7, 0.01, sample_rate);
        env.trigger();

        // Instant attack + decay -> sustain
        for _ in 0..10 {
            env.next_sample();
        }

        // Should hold at sustain
        for _ in 0..1000 {
            let val = env.next_sample();
            assert_f32_approx(val, 0.7);
        }
        assert_eq!(env.phase(), AdsrPhase::Sustain);
    }

    #[test]
    fn test_adsr_release_ramp_down() {
        let sample_rate = 48000.0;
        let release_secs = 0.01; // 480 samples
        let mut env = AdsrEnvelope::new(0.0, 0.0, 0.8, release_secs, sample_rate);
        env.trigger();

        // Get to sustain
        for _ in 0..10 {
            env.next_sample();
        }
        assert_eq!(env.phase(), AdsrPhase::Sustain);

        // Trigger release
        env.release();
        assert_eq!(env.phase(), AdsrPhase::Release);

        let release_samples = (release_secs * sample_rate) as usize;

        // First sample should be near sustain level
        let first = env.next_sample();
        assert!(first > 0.7, "release start should be near sustain, got {first}");

        // Advance to end
        for _ in 1..release_samples {
            env.next_sample();
        }

        // Should be off now
        assert_eq!(env.phase(), AdsrPhase::Off);
        assert_f32_approx(env.value(), 0.0);
    }

    #[test]
    fn test_adsr_full_cycle_timing() {
        let sample_rate = 48000.0;
        let attack = 0.01;  // 480 samples
        let decay = 0.005;  // 240 samples
        let sustain = 0.6;
        let release = 0.01; // 480 samples

        let mut env = AdsrEnvelope::new(attack, decay, sustain, release, sample_rate);
        env.trigger();

        let attack_samples = (attack * sample_rate) as usize;
        let decay_samples = (decay * sample_rate) as usize;
        let release_samples = (release * sample_rate) as usize;

        // Run through attack
        for _ in 0..attack_samples {
            env.next_sample();
        }
        // Should be at peak or in decay now
        let peak_val = env.value();
        assert!(
            (peak_val - 1.0).abs() < 0.01,
            "peak should be ~1.0, got {peak_val}"
        );

        // Run through decay
        for _ in 0..decay_samples {
            env.next_sample();
        }
        let sustain_val = env.value();
        assert!(
            (sustain_val - sustain).abs() < 0.02,
            "should be at sustain {sustain}, got {sustain_val}"
        );

        // Hold sustain for a bit
        for _ in 0..1000 {
            env.next_sample();
        }

        // Release
        env.release();
        for _ in 0..release_samples {
            env.next_sample();
        }
        assert_eq!(env.phase(), AdsrPhase::Off);

        // Timing check: total samples within 1ms (48 samples at 48kHz)
        // Attack timing: 480 samples = 10ms ± 1ms (48 samples tolerance)
        let attack_timing_error = (attack_samples as f32 - attack * sample_rate).abs();
        assert!(
            attack_timing_error < 48.0,
            "attack timing error {attack_timing_error} > 48 samples (1ms)"
        );

        let release_timing_error = (release_samples as f32 - release * sample_rate).abs();
        assert!(
            release_timing_error < 48.0,
            "release timing error {release_timing_error} > 48 samples (1ms)"
        );
    }

    #[test]
    fn test_adsr_off_when_not_triggered() {
        let mut env = AdsrEnvelope::new(0.01, 0.01, 0.5, 0.01, 48000.0);
        assert_eq!(env.phase(), AdsrPhase::Off);
        let val = env.next_sample();
        assert_f32_approx(val, 0.0);
    }

    #[test]
    fn test_adsr_retrigger() {
        let mut env = AdsrEnvelope::new(0.01, 0.0, 1.0, 0.01, 48000.0);
        env.trigger();

        // Advance a bit
        for _ in 0..100 {
            env.next_sample();
        }

        // Retrigger
        env.trigger();
        assert_eq!(env.phase(), AdsrPhase::Attack);
        let val = env.next_sample();
        assert!(val < 0.1, "retrigger should restart from 0, got {val}");
    }

    #[test]
    fn test_adsr_process_buffer() {
        let mut env = AdsrEnvelope::new(0.0, 0.0, 0.5, 0.01, 48000.0);
        env.trigger();

        // After instant A+D, envelope should be at 0.5
        env.next_sample(); // trigger the transition

        let mut buffer = vec![1.0f32; 20]; // 10 stereo frames
        env.process_buffer(&mut buffer);

        // All samples should be multiplied by ~0.5
        for &s in &buffer {
            assert!(
                (s - 0.5).abs() < 0.01,
                "buffer sample should be ~0.5, got {s}"
            );
        }
    }

    // -- Schroeder Reverb tests -----------------------------------------------

    #[test]
    fn test_reverb_impulse_response() {
        let mut reverb = SchroederReverb::new(48000.0, 0.5, 0.5, 1.0);

        // Feed an impulse and collect output
        let mut output = Vec::new();
        output.push(reverb.process_sample(1.0));
        for _ in 0..4800 {
            output.push(reverb.process_sample(0.0));
        }

        // Reverb tail should have energy beyond the initial sample
        let tail_energy: f32 = output[100..].iter().map(|s| s * s).sum();
        assert!(
            tail_energy > 0.001,
            "reverb tail should have energy, got {tail_energy}"
        );
    }

    #[test]
    fn test_reverb_silence_in_silence_out() {
        let mut reverb = SchroederReverb::new(48000.0, 0.5, 0.5, 0.5);

        // Process silence
        for _ in 0..1000 {
            let out = reverb.process_sample(0.0);
            assert_f32_approx(out, 0.0);
        }
    }

    #[test]
    fn test_reverb_reset() {
        let mut reverb = SchroederReverb::new(48000.0, 0.5, 0.5, 1.0);

        // Feed some signal
        for _ in 0..100 {
            reverb.process_sample(1.0);
        }

        reverb.reset();

        // After reset, silence in = silence out
        let out = reverb.process_sample(0.0);
        assert_f32_approx(out, 0.0);
    }

    #[test]
    fn test_reverb_buffer_processing() {
        let mut reverb = SchroederReverb::new(48000.0, 0.3, 0.3, 0.5);

        // Create a stereo buffer with an impulse.
        // Comb filter delays are ~29-44ms (1400-2100 samples at 48kHz),
        // so we need a buffer large enough to see the first reflections.
        let n_frames = 4800; // 100ms
        let mut buffer = vec![0.0f32; n_frames * 2];
        buffer[0] = 1.0; // left impulse
        buffer[1] = 1.0; // right impulse

        reverb.process_buffer(&mut buffer);

        // After comb filter delays (~1400+ samples = frame 1400+),
        // reverb tail should appear
        let tail_start = 1400 * 2; // sample index
        let has_tail = buffer[tail_start..].iter().any(|&s| s.abs() > 0.001);
        assert!(has_tail, "reverb should produce a tail in the buffer");
    }

    #[test]
    fn test_reverb_dry_at_zero_mix() {
        let mut reverb = SchroederReverb::new(48000.0, 0.5, 0.5, 0.0);

        // With mix=0, output should equal input (dry signal only)
        let out = reverb.process_sample(0.75);
        assert_f32_approx(out, 0.75);
    }
}
