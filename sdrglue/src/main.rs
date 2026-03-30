
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

fn main() {
    tracing_subscriber::fmt().init();

    let cli = configuration::Cli::parse();

    let mut fft_planner = rustfft::FftPlanner::new();

    let mut sdr = soapyio::io::SoapyIo::new(&cli.sdr_parameters()).unwrap();

    let mut rx = if sdr.rx_enabled() {
        let mut rx_dsp = rx_dsp::RxDsp::new(
            &mut fft_planner,
            &cli.rx_dsp_parameters(
                sdr.rx_sample_rate(),
                sdr.rx_center_frequency().unwrap(),
            ),
        );
        cli.add_rx_processors(&mut fft_planner, &mut rx_dsp);

        let block_size = rx_dsp.input_block_size();
        Some((rx_dsp, soapyio::block_io::BlockRx::new(block_size)))
    } else {
        None
    };

    let mut tx = if sdr.tx_enabled() {
        let mut tx_dsp = tx_dsp::TxDsp::new(
            &mut fft_planner,
            &cli.tx_dsp_parameters(
                sdr.tx_sample_rate(),
                sdr.tx_center_frequency().unwrap(),
            ),
        );
        cli.add_tx_processors(&mut fft_planner, &mut tx_dsp);

        let block_size = tx_dsp.output_block_size();
        Some((tx_dsp, soapyio::block_io::BlockTx::new(block_size, cli.rx_tx_delay_blocks)))
    } else {
        None
    };

    // Activate SDR once all processors are ready
    sdr.activate().unwrap();

    loop {
        if let Some((rx_dsp, block_rx)) = &mut rx {
            let (buffer, block_count) = block_rx.receive(&mut sdr).unwrap();
            rx_dsp.process(buffer, block_count);
        }

        if let Some((tx_dsp, block_tx)) = &mut tx {
            loop {
                if let Some(block_count) = block_tx.next_block(&mut sdr).unwrap() {
                    block_tx.transmit(&mut sdr, tx_dsp.process(block_count)).unwrap();
                } else {
                    break;
                }
            }
        }

        if rx.is_none() && tx.is_none() {
            tracing::error!("RX and TX are both disabled. Nothing to do.");
            break;
        }
    }
}
