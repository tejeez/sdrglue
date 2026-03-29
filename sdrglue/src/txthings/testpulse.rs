//! Transmit pulses at a given interval.
//! Useful when used together with SDR loopback and I/Q recorder
//! to check for correct timing relationship between TX and RX.

use crate::dsp_types::*;
use super::TxChannelProcessor;

#[derive(Copy, Clone)]
pub struct TestPulseParameters {
    pub sample_rate: f64,
    pub center_frequency: f64,
    pub interval: SampleCount,
}

pub type TestPulse = TestPulseParameters;

impl TestPulse {
    pub fn new(parameters: &TestPulseParameters) -> Self {
        parameters.clone()
    }
}

impl TxChannelProcessor for TestPulse {
    fn process(&mut self, count: SampleCount, samples: &mut [ComplexSample]) {
        for (offset, sample) in samples.iter_mut().enumerate() {
            let count = count.wrapping_add(offset as SampleCount);
            *sample = if count.rem_euclid(self.interval) == 0 {
                ComplexSample {
                    re: 1.0,
                    im: 0.0,
                }
            } else {
                ComplexSample::ZERO
            }
        }
    }

    fn output_sample_rate(&self) -> f64 {
        self.sample_rate
    }

    fn output_center_frequency(&self) -> f64 {
        self.center_frequency
    }
}
