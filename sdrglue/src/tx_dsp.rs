
use rustfft;
use crate::dsp_types::*;
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
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        synth_params: fcfb::SynthesisOutputParameters,
        processor: Box<dyn txthings::TxChannelProcessor>,
    ) -> Self {
        let fcfb_input = fcfb::SynthesisInputProcessor::new_with_frequency(
            fft_planner,
            synth_params,
            processor.output_sample_rate(),
            processor.output_center_frequency(),
            // TODO: maybe add processor.output_bandwidth()
            None,
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
        block_count: fcfb::BlockCount,
    ) {
        self.processor.process(self.synth_input.input_sample_counter(block_count), self.buffer.prepare_for_new_samples());
        synth.add(self.synth_input.process(self.buffer.buffer(), block_count));
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

pub struct TxDspParameters {
    /// Output sample rate (Hz)
    pub sample_rate: f64,
    /// Output sample rate (Hz)
    pub center_frequency: f64,
    /// FCFB bin spacing (Hz)
    pub bin_spacing: f64,
    /// FCFB overlap factor
    pub overlap: fcfb::Overlap,
}

impl TxDsp {
    pub fn new(
        fft_planner: &mut FftPlanner,
        params: &TxDspParameters,
    ) -> Self {
        let synth_params = fcfb::SynthesisOutputParameters {
            ifft_size: (params.sample_rate / params.bin_spacing).round() as usize,
            sample_rate: params.sample_rate,
            center_frequency: params.center_frequency,
            overlap: params.overlap,
        };
        let synth_bank = fcfb::SynthesisOutputProcessor::new(fft_planner, synth_params);

        Self {
            synth_params,
            synth_bank,
            processors: Vec::new(),
        }
    }

    pub fn add_processor(
        &mut self,
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        processor: Box<dyn txthings::TxChannelProcessor>,
    ) {
        self.processors.push(TxChannel::new(fft_planner, self.synth_params, processor));
    }

    pub fn process(
        &mut self,
        block_count: fcfb::BlockCount,
    ) -> &[ComplexSample] {
        for processor in self.processors.iter_mut() {
            processor.process(&mut self.synth_bank, block_count);
        }
        self.synth_bank.process()
    }
}
