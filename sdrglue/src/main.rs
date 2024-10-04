
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
mod rx_dsp;
mod rxthings;
mod soapyconfig;


fn main() {
    let cli = configuration::Cli::parse();

    let mut fft_planner = rustfft::FftPlanner::new();

    let mut sdr = soapyconfig::SoapyIo::init(&cli).unwrap();

    let mut rx_dsp = rx_dsp::RxDsp::new(
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
