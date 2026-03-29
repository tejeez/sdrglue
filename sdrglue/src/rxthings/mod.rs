//! Receive channel processors.

pub mod demodulator;

pub mod iqrecorder;

use crate::dsp_types::*;

pub trait RxChannelProcessor {
    /// Process a block of input samples.
    fn process(&mut self, sample_counter: SampleCount, samples: &[ComplexSample]);

    /// Return required input sample rate in Hertz.
    fn input_sample_rate(&self) -> f64;

    /// Return required input center frequency in Hertz.
    fn input_center_frequency(&self) -> f64;
}
