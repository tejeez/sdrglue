
use super::TxChannelProcessor;
use crate::dsp_types::*;
use std::os::unix::net::UnixDatagram;

const SAMPLE_RATE: f64 = 72000.0;

#[derive(Copy, Clone)]
pub enum Mode {
    /// I/Q at 72 kHz, 32+32-bit floating point
    IQ,
    /// FM baseband and RSSI values at 24 kHz, 16+16-bit unsigned, 0-4095
    FM,
}

impl Mode {
    fn bytes_per_sample(self) -> usize {
        match self {
            Mode::IQ => 8,
            Mode::FM => 4,
        }
    }
}

/// Position of sample count in packet (in bytes)
const SAMPLE_COUNT_POS: usize = 8;

/// Position of the first sample in a packet (in bytes)
const FIRST_SAMPLE_POS: usize = 16;

pub struct TxFromDgram {
    /// Center frequency to demodulate
    center_frequency: f64,
    mode: Mode,
    input_buffer: Vec<u8>,
    packet_info: Option<PacketInfo>,
    /// Socket to send demodulated signal to.
    socket: UnixDatagram,
}

#[derive(Debug)]
struct PacketInfo {
    /// Sample count for first sample in packet
    count: SampleCount,
    /// Number of samples in packet
    nsamples: usize,
}

pub struct TxFromDgramParameters<'a> {
    /// Center frequency to demodulate
    pub center_frequency: f64,
    /// Mode
    pub mode: Mode,
    /// Path of socket
    pub path: &'a str,
}

const PACKET_MAX_BYTES: usize = 0x8000;

impl TxFromDgram {
    pub fn new(parameters: &TxFromDgramParameters) -> Self {
        Self {
            center_frequency: parameters.center_frequency,
            mode: parameters.mode,
            input_buffer: vec![0; PACKET_MAX_BYTES],
            packet_info: None,
            socket: {
                // There may be a leftover socket from a previous run, so first remove it.
                // Ignore return value since it fails if there was no leftover socket.
                let _ = std::fs::remove_file(parameters.path);
                let socket = UnixDatagram::bind(parameters.path).unwrap();
                socket.set_nonblocking(true).unwrap();
                socket
            },
        }
    }

    fn receive_packet(&mut self) {
        self.packet_info = match self.socket.recv(&mut self.input_buffer[..]) {
            Ok(len) => {
                if len >= FIRST_SAMPLE_POS {
                    let packet_info = Some(PacketInfo {
                        count: SampleCount::from_le_bytes(self.input_buffer[SAMPLE_COUNT_POS .. SAMPLE_COUNT_POS+8].try_into().unwrap()),
                        nsamples: (len - FIRST_SAMPLE_POS) / self.mode.bytes_per_sample(),
                    });
                    //tracing::trace!("Packet info: {:?}", packet_info);
                    packet_info
                } else {
                    // Discard invalid packet
                    None
                }
            },
            // No packet available
            _ => None
        };
    }
}

impl TxChannelProcessor for TxFromDgram {
    fn process(&mut self, count: SampleCount, samples: &mut [ComplexSample]) {
        let mut receive_packet_failed = false;
        for (offset, sample) in samples.iter_mut().enumerate() {
            let count = count.wrapping_add(offset as SampleCount);

            *sample = loop {
                if let Some(packet) = &self.packet_info {
                    let sample_offset_in_packet = count.wrapping_sub(packet.count);
                    if sample_offset_in_packet < 0 {
                        // Packet in future, transmit silence until then
                        break ComplexSample::ZERO;
                    } else if sample_offset_in_packet < packet.nsamples as SampleCount {
                        let sample_pos = FIRST_SAMPLE_POS + sample_offset_in_packet as usize * self.mode.bytes_per_sample();
                        let sample_bytes = &self.input_buffer[sample_pos .. sample_pos + self.mode.bytes_per_sample()];
                        break match self.mode {
                            Mode::IQ => ComplexSample {
                                re: RealSample::from_le_bytes(sample_bytes[0 .. 4].try_into().unwrap()),
                                im: RealSample::from_le_bytes(sample_bytes[4 .. 8].try_into().unwrap()),
                            },
                            Mode::FM => todo!(),
                        }
                    } else {
                        // Packet is in the past.
                        // Continue loop to check for a new packet.
                        self.packet_info = None;
                        //eprintln!("Looking for another packet");
                    }
                } else {
                    // Allow at most one failed attempt to read an input packet during a block,
                    // so we don't get stuck in an infinite loop if no data is available.
                    if receive_packet_failed {
                        // Fill the rest of the buffer with zeros
                        break ComplexSample::ZERO;
                    } else {
                        self.receive_packet();
                        if self.packet_info.is_none() {
                            receive_packet_failed = true;
                        }
                        // Continue loop with a new packet if available
                    }
                }
            }
        }
    }

    fn output_sample_rate(&self) -> f64 {
        SAMPLE_RATE
    }

    fn output_center_frequency(&self) -> f64 {
        self.center_frequency
    }
}
