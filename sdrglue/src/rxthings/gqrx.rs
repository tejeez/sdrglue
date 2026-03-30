//! Send demodulated audio to a socket
//! in a format compatible with that sent by Gqrx:
//! signed 16-bit little endian at a sample rate of 48 kHz.
//! This is supported by applications such as Direwolf and Horus GUI.

use super::RxChannelProcessor;
use crate::dsp_types::*;
use crate::filter;
use crate::rxthings::packet_output::PacketOutput;

const SAMPLE_RATE: f64 = 48000.0;

#[derive(Copy, Clone)]
pub enum Modulation {
    FM,
    USB,
    LSB,
}

pub struct Gqrx {
    center_frequency: f64,
    modulation: Modulation,
    flush_each_block: bool,
    /// Previous sample, used for FM demodulation
    previous_sample: ComplexSample,
    /// Used for SSB demodulation.
    second_mixer_phase: usize,
    /// Channel filter, used for both FM and SSB
    /// but with different bandwidth.
    channel_filter: filter::FirComplexSymWithTaps,

    output: PacketOutput,
}

pub struct GqrxParameters<'a> {
    /// Carrier frequency to demodulate
    pub center_frequency: f64,
    /// Address to send UDP packets to.
    pub address: &'a str,
    /// Modulation
    pub modulation: Modulation,
    /// If true, send a packet for each processing block, minimizing latency.
    /// If false, fill up each packet to its maximum size,
    /// possibly reducing CPU use for non-latency-critical applications.
    pub flush_each_block: bool,
}

impl Gqrx {
    pub fn new(parameters: &GqrxParameters) -> Self {
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
            modulation: parameters.modulation,
            flush_each_block: parameters.flush_each_block,

            previous_sample: ComplexSample::ZERO,
            second_mixer_phase: 0,

            // Channels filters are the same for all instances with the same modulation,
            // so memory use could be reduced (which might be good for cache)
            // by computing them once and sharing them among demodulators.
            // This can be done later.
            channel_filter: filter::FirComplexSymWithTaps::new(match parameters.modulation {
                Modulation::FM =>
                    filter::design_fir_lowpass(SAMPLE_RATE, 8000.0, 32),
                Modulation::USB | Modulation::LSB =>
                    filter::design_fir_lowpass(SAMPLE_RATE, 1200.0, 128),
            }),

            // Maximum of 1152 bytes is 12 ms per packet,
            // aligning packets to processing blocks of both 1 ms and 1.5 ms.
            // This is not really important but why not.
            // TODO: return errors
            output: PacketOutput::new(parameters.address, 1152, false).unwrap(),
        }
    }
}

impl RxChannelProcessor for Gqrx {
    fn process(&mut self, _sample_counter: SampleCount, samples: &[ComplexSample]) {
        for &sample in samples {
            let full_scale = i16::MAX as RealSample;

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
            let output_int = (output.min(full_scale).max(-full_scale)).round() as i16;
            // Sample count is not used here, so its value does not matter
            self.output.add_sample(0, &output_int.to_le_bytes());
        }

        if self.flush_each_block {
            self.output.send();
        } else {
            self.output.send_if_full();
        }
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
