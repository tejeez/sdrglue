
pub use rustfft::num_complex as num_complex;
pub use rustfft::num_traits as num_traits;
/// Floating point type used for signal processing.
pub type Sample = f32;
/// Complex floating point type used for signal processing.
pub type ComplexSample = num_complex::Complex<Sample>;
/// Mathematical consts for the Sample type.
pub use std::f32::consts as sample_consts;

mod configuration;
use configuration::Parser;
mod fcfb;
mod rxthings;
mod soapyconfig;

struct RxChannel {
    fcfb_output: fcfb::AnalysisOutputProcessor,
    processor: Box<dyn rxthings::RxChannelProcessor>,
}

impl RxChannel {
    fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        analysis_in_params: fcfb::AnalysisInputParameters,
        processor: Box<dyn rxthings::RxChannelProcessor>,
    ) -> Self {
        Self {
            fcfb_output: fcfb::AnalysisOutputProcessor::new_with_frequency(
                fft_planner,
                analysis_in_params,
                processor.input_sample_rate(),
                processor.input_center_frequency(),
            ),
            processor,
        }
    }

    fn process(
        &mut self,
        intermediate_result: &fcfb::AnalysisIntermediateResult
    ) {
        self.processor.process(self.fcfb_output.process(intermediate_result));
    }
}

/// Everything related to received signal processing.
struct RxDsp {
    /// Input parameters for analysis filter bank.
    analysis_params: fcfb::AnalysisInputParameters,
    /// Analysis filter bank for received signal.
    analysis_bank: fcfb::AnalysisInputProcessor,
    /// Input buffer for signal from SDR to filter bank.
    input_buffer: fcfb::InputBuffer,
    /// Receive channel processors.
    processors: Vec<RxChannel>,
}

impl RxDsp {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        cli: &configuration::Cli,
        sdr_rx_sample_rate: f64,
        sdr_rx_center_frequency: f64,
    ) -> Self {
        let bin_spacing = cli.rx_bin_spacing;

        let analysis_params = fcfb::AnalysisInputParameters {
            fft_size: (sdr_rx_sample_rate / bin_spacing).round() as usize,
            sample_rate: sdr_rx_sample_rate,
            center_frequency: sdr_rx_center_frequency,
        };
        let analysis_bank = fcfb::AnalysisInputProcessor::new(fft_planner, analysis_params);
        let input_buffer = analysis_bank.make_input_buffer();
        Self {
            analysis_params,
            analysis_bank,
            input_buffer,
            processors: Vec::new(),
        }
    }

    pub fn add_processors_from_cli(
        &mut self,
        fft_planner: &mut rustfft::FftPlanner<Sample>,
        cli: &configuration::Cli
    ) {
        for args in cli.demodulate_to_udp.chunks_exact(3) {
            self.processors.push(RxChannel::new(
                fft_planner,
                self.analysis_params,
                Box::new(rxthings::DemodulateToUdp::new(&rxthings::DemodulateToUdpParameters {
                    center_frequency: args[1].parse().unwrap(),
                    address: args[0].as_str(),
                    // TODO: different modulations
                })),
            ));
        }
    }

    pub fn prepare_input_buffer(
        &mut self,
    ) -> &mut [ComplexSample] {
        self.input_buffer.prepare_for_new_samples()
    }

    pub fn process(
        &mut self,
    ) {
        let ir = self.analysis_bank.process(self.input_buffer.buffer());
        for processor in self.processors.iter_mut() {
            processor.process(ir);
        }
    }
}


fn main() {
    let cli = configuration::Cli::parse();

    let mut fft_planner = rustfft::FftPlanner::new();

    let mut sdr = soapyconfig::SoapyIo::init(&cli).unwrap();

    let mut rx_dsp = RxDsp::new(
        &mut fft_planner,
        &cli,
        sdr.rx_sample_rate().unwrap(),
        sdr.rx_center_frequency().unwrap()
    );
    rx_dsp.add_processors_from_cli(&mut fft_planner, &cli);

    let mut error_count = 0;

    loop {
        match sdr.receive(rx_dsp.prepare_input_buffer()) {
            Ok(_rx_result) => {
                error_count = 0;
                rx_dsp.process();
            },
            Err(err) => {
                error_count += 1;
                eprintln!("Error receiving from SDR ({}): {}", error_count, err);
                // Occasional errors might sometimes occur with some SDRs
                // even if they would still continue working.
                // If too many reads result in an error with no valid reads
                // in between, assume the SDR is broken and stop.
                if error_count >= 10 {
                    break
                }
            },
        }
    }
}
