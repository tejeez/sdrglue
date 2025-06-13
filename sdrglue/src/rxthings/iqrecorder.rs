
use super::RxChannelProcessor;
use crate::{Sample, ComplexSample, sample_consts};
use std::fs::File;
use std::io::prelude::*;

pub struct RecordIq {
    /// Recording sample rate
    sample_rate: f64,
    /// Center frequency to record
    center_frequency: f64,
    /// Output buffer.
    /// Demodulated signal is written here
    /// in the format that is sent to the UDP socket.
    output_buffer: Vec<u8>,
    /// Output file.
    file: std::fs::File,
}

pub struct RecordIqParameters<'a> {
    /// Recording sample rate
    pub sample_rate: f64,
    /// Center frequency to record
    pub center_frequency: f64,
    /// Output file name.
    pub filename: &'a str,
}

impl RecordIq {
    pub fn new(parameters: &RecordIqParameters) -> Self {
        Self {
            sample_rate: parameters.sample_rate,
            center_frequency: parameters.center_frequency,
            output_buffer: Vec::<u8>::new(),
            // TODO: handle error somehow if creating the file fails
            file: File::create(parameters.filename).unwrap(),
        }
    }
}

impl RxChannelProcessor for RecordIq {
    fn process(&mut self, samples: &[ComplexSample]) {
        self.output_buffer.clear();
        for &sample in samples {
            for value in [sample.re, sample.im] {
                // Write as little endian 32-bit float
                let bits = value.to_bits();
                self.output_buffer.push(bits as u8);
                self.output_buffer.push((bits >> 8) as u8);
                self.output_buffer.push((bits >> 16) as u8);
                self.output_buffer.push((bits >> 24) as u8);
            }
        }
        // TODO: print a warning or something if writing to file fails
        let _ = self.file.write_all(&self.output_buffer);
    }

    fn input_sample_rate(&self) -> f64 {
        self.sample_rate
    }

    fn input_center_frequency(&self) -> f64 {
        self.center_frequency
    }
}
