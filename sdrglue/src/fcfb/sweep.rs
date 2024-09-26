//! Frequency sweep generator, useful for testing.

use crate::{Sample, ComplexSample, sample_consts};

pub struct SweepGenerator {
    /// Length of a sweep in samples.
    sweep_length: u64,
    /// Number of samples produced for current sweep.
    sample_counter: u64,
    /// Phase accumulator.
    phase: Sample,
    /// Initial frequency in radians per sample.
    initial_frequency: Sample,
    /// Rate of change of frequency in radians per sample^2.
    frequency_step: Sample,
}

impl SweepGenerator {
    pub fn new(
        sweep_length: u64
    ) -> Self {
        Self {
            sweep_length,
            sample_counter: 0,
            phase: 0.0,
            initial_frequency: -sample_consts::PI,
            frequency_step: sample_consts::PI * 2.0 / (sweep_length as Sample),
        }
    }

    pub fn sample(&mut self) -> ComplexSample {
        let result = ComplexSample { re: self.phase.cos(), im: self.phase.sin() };
        let freq = self.initial_frequency + self.sample_counter as Sample * self.frequency_step;
        self.phase = (self.phase + freq).rem_euclid(sample_consts::PI * 2.0);
        self.sample_counter += 1;
        if self.sample_counter >= self.sweep_length {
            self.sample_counter = 0;
        }
        result
    }
}
