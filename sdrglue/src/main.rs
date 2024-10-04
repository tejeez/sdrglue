
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

    let mut rx_dsp = if sdr.rx_enabled() {
        Some(rx_dsp::RxDsp::new(
            &mut fft_planner,
            &cli,
            sdr.rx_sample_rate().unwrap(),
            sdr.rx_center_frequency().unwrap()
        ))
    } else {
        None
    };

    let mut error_count = 0;

    loop {
        if let Some(rx_dsp) = &mut rx_dsp {
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

        if rx_dsp.is_none() /* && tx_dsp.is_none() */ {
            eprintln!("RX is disabled. Nothing to do.");
            //eprintln!("RX and TX are both disabled. Nothing to do.");
            break;
        }
    }
}
