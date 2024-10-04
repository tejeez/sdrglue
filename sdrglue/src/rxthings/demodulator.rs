
use super::RxChannelProcessor;
use crate::{Sample, ComplexSample, sample_consts};
use crate::filter;

const SAMPLE_RATE: f64 = 48000.0;

#[derive(Copy, Clone)]
pub enum Modulation {
    FM,
    USB,
    LSB,
}

pub struct DemodulateToUdp {
    /// Center frequency to demodulate
    center_frequency: f64,
    /// Modulation
    modulation: Modulation,
    /// Previous sample, used for FM demodulation
    previous_sample: ComplexSample,
    /// Used for SSB demodulation.
    second_mixer_phase: usize,
    /// Channel filter, used for both FM and SSB
    /// but with different bandwidth.
    channel_filter: filter::FirCf32Sym,
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
            second_mixer_phase: 0,
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
                    let out = (filtered * self.previous_sample.conj()).arg() * (full_scale * sample_consts::FRAC_1_PI);
                    self.previous_sample = filtered;
                    out
                },
                Modulation::USB | Modulation::LSB => {
                    (filtered * SSB_SECOND_MIXER_TABLE[self.second_mixer_phase]).re * full_scale
                },
            };

            // All this SSB stuff could be cleaned up a bit...

            match self.modulation {
                Modulation::USB => {
                    self.second_mixer_phase += 1;
                    if self.second_mixer_phase >= SSB_SECOND_MIXER_TABLE.len() {
                        self.second_mixer_phase = 0;
                    }
                },
                Modulation::LSB => {
                    if self.second_mixer_phase == 0 {
                        self.second_mixer_phase = SSB_SECOND_MIXER_TABLE.len() - 1;
                    } else {
                        self.second_mixer_phase -= 1;
                    }
                },
                _ => {},
            }

            // Format conversion
            let output_int = (output.min(full_scale).max(-full_scale)) as i16;
            self.output_buffer.push((output_int & 0xFF) as u8);
            self.output_buffer.push((output_int >> 8)   as u8);
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


const SSB_WEAVER_OFFSET: f64 = 1500.0;

/// One cycle of complex sine wave for the second mixer
/// in Weaver method SSB demodulator.
/// Computing it at compile time is not possible for floating point
/// and computing it at run time would unnecessarily complicate the code,
/// so just put the values here.
/// Computed in Python with:
/// import numpy as np
/// for v in np.exp(1j * np.linspace(0, np.pi*2, 32, endpoint=False)):
///  print('    ComplexSample { re: %11.8f, im: %11.8f },' % (v.real, v.imag))
const SSB_SECOND_MIXER_TABLE: [ComplexSample; 32] = [
    ComplexSample { re:  1.00000000, im:  0.00000000 },
    ComplexSample { re:  0.98078528, im:  0.19509032 },
    ComplexSample { re:  0.92387953, im:  0.38268343 },
    ComplexSample { re:  0.83146961, im:  0.55557023 },
    ComplexSample { re:  0.70710678, im:  0.70710678 },
    ComplexSample { re:  0.55557023, im:  0.83146961 },
    ComplexSample { re:  0.38268343, im:  0.92387953 },
    ComplexSample { re:  0.19509032, im:  0.98078528 },
    ComplexSample { re:  0.00000000, im:  1.00000000 },
    ComplexSample { re: -0.19509032, im:  0.98078528 },
    ComplexSample { re: -0.38268343, im:  0.92387953 },
    ComplexSample { re: -0.55557023, im:  0.83146961 },
    ComplexSample { re: -0.70710678, im:  0.70710678 },
    ComplexSample { re: -0.83146961, im:  0.55557023 },
    ComplexSample { re: -0.92387953, im:  0.38268343 },
    ComplexSample { re: -0.98078528, im:  0.19509032 },
    ComplexSample { re: -1.00000000, im:  0.00000000 },
    ComplexSample { re: -0.98078528, im: -0.19509032 },
    ComplexSample { re: -0.92387953, im: -0.38268343 },
    ComplexSample { re: -0.83146961, im: -0.55557023 },
    ComplexSample { re: -0.70710678, im: -0.70710678 },
    ComplexSample { re: -0.55557023, im: -0.83146961 },
    ComplexSample { re: -0.38268343, im: -0.92387953 },
    ComplexSample { re: -0.19509032, im: -0.98078528 },
    ComplexSample { re: -0.00000000, im: -1.00000000 },
    ComplexSample { re:  0.19509032, im: -0.98078528 },
    ComplexSample { re:  0.38268343, im: -0.92387953 },
    ComplexSample { re:  0.55557023, im: -0.83146961 },
    ComplexSample { re:  0.70710678, im: -0.70710678 },
    ComplexSample { re:  0.83146961, im: -0.55557023 },
    ComplexSample { re:  0.92387953, im: -0.38268343 },
    ComplexSample { re:  0.98078528, im: -0.19509032 },
];
