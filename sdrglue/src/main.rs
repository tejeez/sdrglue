
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
mod tx_dsp;
mod rxthings;
mod txthings;
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

    let mut tx_dsp = if sdr.tx_enabled() {
        Some(tx_dsp::TxDsp::new(
            &mut fft_planner,
            &cli,
            sdr.tx_sample_rate().unwrap(),
            sdr.tx_center_frequency().unwrap()
        ))
    } else {
        None
    };

    let mut error_count = 0;

    loop {
        let mut rx_time: Option<i64> = None;

        if let Some(rx_dsp) = &mut rx_dsp {
            match sdr.receive(rx_dsp.prepare_input_buffer()) {
                Ok(rx_result) => {
                    error_count = 0;
                    rx_time = rx_result.time;
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

        if let Some(tx_dsp) = &mut tx_dsp {
            let tx_time: Option<i64> = if let Some(rx_time) = rx_time { Some(rx_time + cli.rx_tx_delay) } else { None };
            match sdr.transmit(tx_dsp.process(), tx_time) {
                Ok(_) => {},
                Err(err) => {
                    error_count += 1;
                    eprintln!("Error transmitting to SDR ({}): {}", error_count, err);
                    if error_count >= 10 {
                        break
                    }
                }
            }
        }

        if rx_dsp.is_none() && tx_dsp.is_none() {
            eprintln!("RX and TX are both disabled. Nothing to do.");
            break;
        }
    }
}
