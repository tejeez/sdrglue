//! Transmit channel processors.

use crate::ComplexSample;

pub trait TxChannelProcessor {
    /// Produce a block of transmit samples.
    /// The function should always fill the whole buffer
    /// with new transmit samples.
    fn process(&mut self, samples: &mut [ComplexSample]);

    /// Return output sample rate in Hertz.
    fn output_sample_rate(&self) -> f64;

    /// Return output center frequency in Hertz.
    fn output_center_frequency(&self) -> f64;
}
