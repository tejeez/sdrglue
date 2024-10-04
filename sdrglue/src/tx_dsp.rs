
use rustfft;
use crate::{Sample, ComplexSample};
use crate::configuration;
use crate::fcfb;
use crate::txthings;


struct TxChannel {
    synth_input: fcfb::SynthesisInputProcessor,
    processor: Box<dyn txthings::TxChannelProcessor>,
    /// Buffer to transfer samples from channel processor to filter bank.
    buffer: fcfb::InputBuffer,
}

impl TxChannel {
    fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        synth_params: fcfb::SynthesisOutputParameters,
        processor: Box<dyn txthings::TxChannelProcessor>,
    ) -> Self {
        let fcfb_input = fcfb::SynthesisInputProcessor::new_with_frequency(
            fft_planner,
            synth_params,
            processor.output_sample_rate(),
            processor.output_center_frequency(),
        );
        let buffer = fcfb_input.make_input_buffer();
        Self {
            synth_input: fcfb_input,
            processor,
            buffer,
        }
    }

    fn process(
        &mut self,
        synth: &mut fcfb::SynthesisOutputProcessor,
    ) {
        self.processor.process(self.buffer.prepare_for_new_samples());
        synth.add(self.synth_input.process(self.buffer.buffer()));
    }
}

/// Everything related to transmit signal processing.
pub struct TxDsp {
    /// Parameters for synthesis filter bank.
    synth_params: fcfb::SynthesisOutputParameters,
    /// Analysis filter bank for received signal.
    synth_bank: fcfb::SynthesisOutputProcessor,
    /// Transmit channel processors.
    processors: Vec<TxChannel>,
}

impl TxDsp {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        cli: &configuration::Cli,
        sdr_tx_sample_rate: f64,
        sdr_tx_center_frequency: f64,
    ) -> Self {
        let bin_spacing = cli.tx_bin_spacing;

        let synth_params = fcfb::SynthesisOutputParameters {
            ifft_size: (sdr_tx_sample_rate / bin_spacing).round() as usize,
            sample_rate: sdr_tx_sample_rate,
            center_frequency: sdr_tx_center_frequency,
        };
        let synth_bank = fcfb::SynthesisOutputProcessor::new(fft_planner, synth_params);

        let mut self_ = Self {
            synth_params,
            synth_bank,
            processors: Vec::new(),
        };
        self_
    }

    pub fn process(
        &mut self,
    ) -> &[ComplexSample] {
        for processor in self.processors.iter_mut() {
            processor.process(&mut self.synth_bank);
        }
        self.synth_bank.process()
    }
}
