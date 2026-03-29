//! Transmit channel processors.

pub mod dgram;
pub mod testpulse;

use crate::dsp_types::*;

pub trait TxChannelProcessor {
    /// Produce a block of transmit samples
    /// starting from a given sample counter value.
    /// The function should always fill the whole buffer
    /// with new transmit samples.
    fn process(&mut self, sample_counter: SampleCount, samples: &mut [ComplexSample]);

    /// Return output sample rate in Hertz.
    fn output_sample_rate(&self) -> f64;

    /// Return output center frequency in Hertz.
    fn output_center_frequency(&self) -> f64;
}
