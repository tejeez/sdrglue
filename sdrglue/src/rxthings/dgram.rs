
use crate::dsp_types::*;
use super::RxChannelProcessor;
use std::os::unix::net::UnixDatagram;

const SAMPLE_RATE: f64 = 72000.0;

#[derive(Copy, Clone)]
pub enum Mode {
    /// I/Q at 72 kHz
    IQ,
    /// FM baseband at 24 kHz
    FM,
}

pub struct RxToDgram {
    /// Center frequency to demodulate
    center_frequency: f64,
    /// Mode
    mode: Mode,
    /// Output buffer.
    /// Demodulated signal is written here
    /// in the format that is sent to the socket.
    output_buffer: Vec<u8>,
    /// Socket to send demodulated signal to.
    socket: UnixDatagram,
}

pub struct RxToDgramParameters<'a> {
    /// Center frequency to demodulate
    pub center_frequency: f64,
    /// Modulation
    pub mode: Mode,
    /// Path of socket
    pub path: &'a str,
}

const PACKET_MAX_BYTES: usize = 0x8000;

impl RxToDgram {
    pub fn new(parameters: &RxToDgramParameters) -> Self {
        Self {
            center_frequency: parameters.center_frequency,
            mode: parameters.mode,
            output_buffer: Vec::<u8>::with_capacity(PACKET_MAX_BYTES),
            socket: {
                let socket = UnixDatagram::unbound().unwrap();
                socket.connect(parameters.path).unwrap();
                socket.set_nonblocking(true).unwrap();
                socket
            }
        }
    }
}

impl RxChannelProcessor for RxToDgram {
    fn process(&mut self, count: SampleCount, samples: &[ComplexSample]) {
        self.output_buffer.clear();
        let header_value: u64 = 0; // placeholder, not decided yet
        self.output_buffer.extend_from_slice(&header_value.to_le_bytes());
        self.output_buffer.extend_from_slice(&count.to_le_bytes());
        for &sample in samples {
            match self.mode {
                Mode::IQ => {
                    self.output_buffer.extend_from_slice(&sample.re.to_le_bytes());
                    self.output_buffer.extend_from_slice(&sample.im.to_le_bytes());
                },
                Mode::FM => todo!(),
            }
        }
        match self.socket.send(&self.output_buffer) {
            Ok(len) => {
                //tracing::trace!("Wrote {}/{} bytes to socket", len, self.output_buffer.len());
            },
            Err(err) => {
                tracing::error!("Error writing to socket: {}", err);
            }
        }
    }

    fn input_sample_rate(&self) -> f64 {
        SAMPLE_RATE
    }

    fn input_center_frequency(&self) -> f64 {
        self.center_frequency
    }
}
