//! Frequency sweep generator, useful for testing.

use rustfft::num_complex::Complex32;

pub struct SweepGenerator {
    /// Length of a sweep in samples.
    sweep_length: u64,
    /// Number of samples produced for current sweep.
    sample_counter: u64,
    /// Phase accumulator.
    phase: f32,
    /// Initial frequency in radians per sample.
    initial_frequency: f32,
    /// Rate of change of frequency in radians per sample^2.
    frequency_step: f32,
}

impl SweepGenerator {
    pub fn new(
        sweep_length: u64
    ) -> Self {
        Self {
            sweep_length,
            sample_counter: 0,
            phase: 0.0,
            initial_frequency: -std::f32::consts::PI,
            frequency_step: std::f32::consts::PI * 2.0 / (sweep_length as f32),
        }
    }

    pub fn sample(&mut self) -> Complex32 {
        let result = Complex32 { re: self.phase.cos(), im: self.phase.sin() };
        let freq = self.initial_frequency + self.sample_counter as f32 * self.frequency_step;
        self.phase = (self.phase + freq).rem_euclid(std::f32::consts::PI * 2.0);
        self.sample_counter += 1;
        if self.sample_counter >= self.sweep_length {
            self.sample_counter = 0;
        }
        result
    }
}
