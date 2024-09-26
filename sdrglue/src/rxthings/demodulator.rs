
use super::RxChannelProcessor;
use crate::{ComplexSample, sample_consts};

pub struct DemodulateToUdp {
    /// Center frequency to demodulate
    center_frequency: f64,
    /// Previous sample, used for FM demodulation
    previous_sample: ComplexSample,
    /// Output buffer.
    /// Demodulated signal is written here
    /// in the format that is sent to the UDP socket.
    output_buffer: Vec<u8>,
    /// Socket to send demodulated signal to.
    socket: std::net::UdpSocket,
}

pub struct DemodulateToUdpParameters<'a> {
    /// Center frequency to demodulate
    pub center_frequency: f64,
    /// Address to send UDP packets to.
    pub address: &'a str,
}

impl DemodulateToUdp {
    pub fn new(parameters: &DemodulateToUdpParameters) -> Self {
        Self {
            center_frequency: parameters.center_frequency,
            previous_sample: ComplexSample::ZERO,
            // Already allocate space for 1 ms block of output signal.
            // Well, the blocks might be longer if bin spacing is reduced,
            // but even if it is, more space will be allocated while
            // processing the first block and no more dynamic allocations
            // are needed after that, so it is not really a problem.
            output_buffer: Vec::<u8>::with_capacity(96),
            socket: {
                // Does the bind address matter if we only send data to the socket?
                // TODO: handle error somehow if creating the socket or connecting fails
                let socket = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
                socket.connect(parameters.address).unwrap();
                socket
            },
        }
    }
}

impl RxChannelProcessor for DemodulateToUdp {
    fn process(&mut self, samples: &[ComplexSample]) {
        self.output_buffer.clear();
        for &sample in samples {
            let full_scale = 32767.0;
            let demodulated_fm = ((sample * self.previous_sample.conj()).arg() * (full_scale * sample_consts::FRAC_1_PI)) as i16;
            self.output_buffer.push((demodulated_fm & 0xFF) as u8);
            self.output_buffer.push((demodulated_fm >> 8)   as u8);
            self.previous_sample = sample;
        }
        // TODO: print a warning or something if writing to socket fails
        let _ = self.socket.send(&self.output_buffer);
    }

    fn input_sample_rate(&self) -> f64 {
        48000.0
    }

    fn input_center_frequency(&self) -> f64 {
        self.center_frequency
    }
}
