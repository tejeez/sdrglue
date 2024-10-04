
use super::RxChannelProcessor;
use crate::{Sample, ComplexSample, sample_consts};
use crate::filter;

const SAMPLE_RATE: f64 = 48000.0;
const SSB_WEAVER_OFFSET: f64 = 1500.0;

#[derive(Copy, Clone)]
pub enum Modulation {
    FM,
    USB,
    LSB,
}

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

    channel_filter: filter::FirCf32Sym,
    modulation: Modulation,
}

pub struct DemodulateToUdpParameters<'a> {
    /// Center frequency to demodulate
    pub center_frequency: f64,
    /// Address to send UDP packets to.
    pub address: &'a str,
    /// Modulation
    pub modulation: Modulation,
}

impl DemodulateToUdp {
    pub fn new(parameters: &DemodulateToUdpParameters) -> Self {
        Self {
            center_frequency:
                parameters.center_frequency
                + match parameters.modulation {
                    Modulation::FM => 0.0,
                    // Weaver method SSB: offset downconverter so we can
                    // use a channel filter with real-valued taps.
                    Modulation::USB =>  SSB_WEAVER_OFFSET,
                    Modulation::LSB => -SSB_WEAVER_OFFSET,
                },
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
            // Channels filters are the same for all instances with the same modulation,
            // so memory use could be reduced (which might be good for cache)
            // by computing them once and sharing them among demodulators.
            // This can be done later.
            channel_filter: filter::FirCf32Sym::new(match parameters.modulation {
                Modulation::FM =>
                    filter::design_fir_lowpass(SAMPLE_RATE, 8000.0, 32),
                Modulation::USB | Modulation::LSB =>
                    filter::design_fir_lowpass(SAMPLE_RATE, 1200.0, 128),
            }),
            modulation: parameters.modulation,
        }
    }
}

impl RxChannelProcessor for DemodulateToUdp {
    fn process(&mut self, samples: &[ComplexSample]) {
        self.output_buffer.clear();
        for &sample in samples {
            let full_scale = i16::MAX as Sample;

            let filtered = self.channel_filter.sample(sample);

            let output = match self.modulation {
                Modulation::FM => {
                    (filtered * self.previous_sample.conj()).arg() * (full_scale * sample_consts::FRAC_1_PI)
                },
                Modulation::USB | Modulation::LSB => {
                    // TODO: second mixer of Weaver method
                    filtered.re * full_scale
                },
            };

            // Format conversion
            let output_int = (output.min(full_scale).max(-full_scale)) as i16;
            self.output_buffer.push((output_int & 0xFF) as u8);
            self.output_buffer.push((output_int >> 8)   as u8);
            self.previous_sample = sample;
        }
        // TODO: print a warning or something if writing to socket fails
        let _ = self.socket.send(&self.output_buffer);
    }

    fn input_sample_rate(&self) -> f64 {
        SAMPLE_RATE
    }

    fn input_center_frequency(&self) -> f64 {
        self.center_frequency
    }
}
