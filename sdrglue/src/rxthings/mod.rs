//! Receive channel processors.

use crate::ComplexSample;

pub mod demodulator;
pub use demodulator::*;

pub trait RxChannelProcessor {
    /// Process a block of input samples.
    fn process(&mut self, samples: &[ComplexSample]);

    /// Return required input sample rate in Hertz.
    fn input_sample_rate(&self) -> f64;

    /// Return required input center frequency in Hertz.
    fn input_center_frequency(&self) -> f64;
}
