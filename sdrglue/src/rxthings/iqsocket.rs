//! Send I/Q to a socket as interleaved little endian floating point numbers

use crate::dsp_types::*;
use super::RxChannelProcessor;
use super::packet_output::PacketOutput;

pub struct IqSocket {
    pub sample_rate: f64,
    center_frequency: f64,
    output: PacketOutput,
}

pub struct IqSocketParameters<'a> {
    /// Sample rate for output I/Q stream
    pub sample_rate: f64,
    /// Center frequency for output I/Q stream
    pub center_frequency: f64,
    /// Address or path for output socket
    pub address: &'a str,
    /// Add a header with sample count in each packet
    pub use_header: bool,
}

impl IqSocket {
    pub fn new(parameters: &IqSocketParameters) -> Self {
        Self {
            sample_rate: parameters.sample_rate,
            center_frequency: parameters.center_frequency,
            // TODO: return errors
            output: PacketOutput::new(parameters.address, 1024, parameters.use_header).unwrap(),
        }
    }
}

impl RxChannelProcessor for IqSocket {
    fn process(&mut self, count: SampleCount, samples: &[ComplexSample]) {
        for &sample in samples {
            self.output.add_sample2(count, &sample.re.to_le_bytes(), &sample.im.to_le_bytes());
        }
        self.output.send();
    }

    fn input_sample_rate(&self) -> f64 {
        self.sample_rate
    }

    fn input_center_frequency(&self) -> f64 {
        self.center_frequency
    }
}
