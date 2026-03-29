
mod configuration;
use configuration::Parser;
mod dsp_types;
mod fcfb;
mod filter;
mod rx_dsp;
mod tx_dsp;
mod rxthings;
mod txthings;
mod soapyio;
mod test_timing;

use dsp_types::*;


fn main() {
    tracing_subscriber::fmt().init();

    let cli = configuration::Cli::parse();

    let mut fft_planner = rustfft::FftPlanner::new();

    let mut sdr = soapyio::io::SoapyIo::new(&cli.sdr_parameters()).unwrap();

    let mut rx_dsp = if sdr.rx_enabled() {
        let mut rx_dsp = rx_dsp::RxDsp::new(
            &mut fft_planner,
            &cli.rx_dsp_parameters(
                sdr.rx_sample_rate(),
                sdr.rx_center_frequency().unwrap(),
            ),
        );
        cli.add_rx_processors(&mut fft_planner, &mut rx_dsp);
        Some(rx_dsp)
    } else {
        None
    };

    let mut tx_dsp = if sdr.tx_enabled() {
        let mut tx_dsp = tx_dsp::TxDsp::new(
            &mut fft_planner,
            &cli.tx_dsp_parameters(
                sdr.tx_sample_rate(),
                sdr.tx_center_frequency().unwrap(),
            ),
        );
        cli.add_tx_processors(&mut fft_planner, &mut tx_dsp);
        Some(tx_dsp)
    } else {
        None
    };

    let mut error_count = 0;
    let mut rx_block_count = 0;
    let mut minimum_timing_margin: SampleCount = SampleCount::MAX;

    loop {
        let mut rx_sample_count: SampleCount = 0;

        if let Some(rx_dsp) = &mut rx_dsp {
            let input_buffer = rx_dsp.prepare_input_buffer();
            match sdr.receive(input_buffer) {
                Ok(rx_result) => {
                    error_count = 0;
                    assert!(rx_result.len == input_buffer.len(), "Short RX reads are not handled yet");
                    rx_sample_count = rx_result.count;
                    rx_dsp.process(rx_block_count);
                    // TODO: handle lost samples.
                    // Now assuming RX signal is contiguous.
                    rx_block_count += 1;
                },
                Err(_) => {
                    error_count += 1;
                    tracing::error!("Error receiving from SDR ({})", error_count);
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
            let tx_block_count = rx_block_count.wrapping_add(cli.rx_tx_delay_blocks);
            let tx_sample_count = rx_sample_count.wrapping_add(cli.rx_tx_delay_blocks * tx_dsp.output_block_size() as SampleCount);
            match sdr.transmit(tx_dsp.process(tx_block_count), Some(tx_sample_count)) {
                Ok(_) => {
                    let current_count = sdr.tx_current_count().unwrap();
                    let timing_margin = tx_sample_count.wrapping_sub(current_count);
                    minimum_timing_margin = minimum_timing_margin.min(timing_margin);
                    if tx_block_count.rem_euclid(1024) == 0 {
                        let minimum_timing_margin_ms = 1000.0 * minimum_timing_margin as f64 / sdr.tx_sample_rate();
                        tracing::info!("Estimated margin for TX deadline: {:.2} ms", minimum_timing_margin_ms);
                        minimum_timing_margin = timing_margin;
                    }
                },
                Err(_) => {
                    error_count += 1;
                    tracing::error!("Error transmitting to SDR ({})", error_count);
                    if error_count >= 10 {
                        break
                    }
                }
            }
        }

        if rx_dsp.is_none() && tx_dsp.is_none() {
            tracing::error!("RX and TX are both disabled. Nothing to do.");
            break;
        }
    }
}
