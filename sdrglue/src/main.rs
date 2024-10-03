
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

fn main() {
    let cli = configuration::Cli::parse();

    // Spacing of FCFB FFT bins in Hz
    let bin_spacing = 500.0;

    let mut sdr = soapyconfig::SoapyIo::init(&cli).unwrap();

    let sdr_rx_sample_rate = sdr.rx_sample_rate().unwrap();
    let sdr_rx_center_frequency = sdr.rx_center_frequency().unwrap();

    let analysis_in_params = fcfb::AnalysisInputParameters {
        fft_size: (sdr_rx_sample_rate / bin_spacing).round() as usize,
        input_sample_rate: sdr_rx_sample_rate,
        input_center_frequency: sdr_rx_center_frequency,
    };
    let mut fft_planner = rustfft::FftPlanner::new();
    let mut analysis_bank = fcfb::AnalysisInputProcessor::new(&mut fft_planner, analysis_in_params);
    let mut rx_buffer = analysis_bank.make_input_buffer();

    let mut rx_processors = Vec::<RxChannel>::new();

    for args in cli.demodulate_to_udp.chunks_exact(3) {
        rx_processors.push(RxChannel::new(
            &mut fft_planner,
            analysis_in_params,
            Box::new(rxthings::DemodulateToUdp::new(&rxthings::DemodulateToUdpParameters {
                center_frequency: args[1].parse().unwrap(),
                address: args[0].as_str(),
                // TODO: different modulations
            })),
        ));
    }

    let mut error_count = 0;

    loop {
        match sdr.receive(rx_buffer.prepare_for_new_samples()) {
            Ok(_rx_result) => {
                error_count = 0;
                let ir = analysis_bank.process(rx_buffer.buffer());
                for processor in rx_processors.iter_mut() {
                    processor.process(ir);
                }
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
