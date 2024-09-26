use soapysdr;
use crate::configuration;

type StreamType = crate::ComplexSample;

struct SdrDefaults<'a> {
    /// Receive sample rate
    pub rx_fs: f64,
    /// Transmit sample rate
    pub tx_fs: f64,
    /// Receive antenna
    pub rx_ant:   Option<&'a str>,
    /// Transmit antenna
    pub tx_ant:   Option<&'a str>,
    /// Receive gain(s).
    /// Use (None, gain_value) to set the overall gain.
    /// Use ("name", gain_value) to set a specific gain element.
    pub rx_gain:  &'a[(Option<&'a str>, f64)],
    /// Transmit gain(s).
    pub tx_gain:  &'a[(Option<&'a str>, f64)],
}

pub const LIMESDR_DEFAULT: SdrDefaults = SdrDefaults {
    rx_fs: 8192e3,
    tx_fs: 8192e3,
    rx_ant: Some("LNAL"),
    tx_ant: Some("BAND1"),
    rx_gain: &[
        (Some("LNA"), 20.0),
        (Some("TIA"), 10.0),
        (Some("PGA"), 10.0),
    ],
    tx_gain: &[
        (Some("PAD" ), 52.0),
        (Some("IAMP"), 3.0),
    ],
};

pub const SXCEIVER_DEFAULT: SdrDefaults = SdrDefaults {
    rx_fs: 600e3,
    tx_fs: 600e3,
    rx_ant: Some("RX"),
    tx_ant: Some("TX"),
    rx_gain: &[
        (Some("LNA"), 42.0),
        (Some("PGA"), 16.0),
    ],
    tx_gain: &[
        (Some("DAC"  ), 9.0),
        (Some("MIXER"), 30.0),
    ],
};

pub struct SoapyIo {
    rx_ch:  usize,
    tx_ch:  usize,
    dev: soapysdr::Device,
    /// Receive stream. None if receiving is disabled.
    rx:  Option<soapysdr::RxStream<StreamType>>,
    /// Transmit stream. None if transmitting is disabled.
    tx:  Option<soapysdr::TxStream<StreamType>>,
}

/// Convert command line device arguments to soapysdr::Args.
fn convert_args(cli_args: &[String]) -> soapysdr::Args {
    let mut args = soapysdr::Args::new();
    for arg in cli_args.chunks_exact(2) {
        args.set(arg[0].as_str(), arg[1].as_str());
    }
    args
}

/// It is annoying to repeat error handling so do that in a macro.
/// ? could be used but then it could not print which SoapySDR call failed.
macro_rules! soapycheck {
    ($text:literal, $soapysdr_call:expr) => {
        match $soapysdr_call {
            Ok(ret) => { ret },
            Err(err) => {
                eprintln!("SoapySDR: Failed to {}: {}", $text, err);
                return Err(err);
            }
        }
    }
}

impl SoapyIo {
    pub fn init(cli: &configuration::Cli) -> Result<Self, soapysdr::Error> {
        let rx_ch = cli.sdr_rx_ch;
        let tx_ch = cli.sdr_tx_ch;

        let dev = soapycheck!("open SoapySDR device",
            soapysdr::Device::new(convert_args(&cli.sdr_device)));

        let rx_enabled = cli.sdr_rx_freq.is_some();
        let tx_enabled = cli.sdr_rx_freq.is_some();

        let sdr_defaults = LIMESDR_DEFAULT; // TODO: choose correct defaults

        if rx_enabled {
            soapycheck!("set RX sample rate",
                dev.set_sample_rate(soapysdr::Direction::Rx, rx_ch, cli.sdr_rx_fs.unwrap_or(sdr_defaults.rx_fs)));
        }
        if tx_enabled {
            soapycheck!("set TX sample rate",
                dev.set_sample_rate(soapysdr::Direction::Tx, tx_ch, cli.sdr_tx_fs.unwrap_or(sdr_defaults.tx_fs)));
        }
        //TODO
        /*
        soapycheck!("set RX center frequency",
            dev.set_frequency(soapysdr::Direction::Rx, conf.rx_chan, conf.rx_freq, soapysdr::Args::new()));
        soapycheck!("set TX center frequency",
            dev.set_frequency(soapysdr::Direction::Tx, conf.tx_chan, conf.tx_freq, soapysdr::Args::new()));
        soapycheck!("set RX antenna",
            dev.set_antenna(soapysdr::Direction::Rx, conf.rx_chan, conf.rx_ant));
        soapycheck!("set TX antenna",
            dev.set_antenna(soapysdr::Direction::Tx, conf.tx_chan, conf.tx_ant));
        for (name, value) in conf.rx_gain {
            if let Some(name) = name {
                soapycheck!("set RX gain element",
                    dev.set_gain_element(soapysdr::Direction::Rx, conf.rx_chan, name.as_bytes().to_vec(), *value));
            } else {
                soapycheck!("set RX overall gain",
                    dev.set_gain(soapysdr::Direction::Rx, conf.rx_chan, *value));
            }
        }
        for (name, value) in conf.tx_gain {
            if let Some(name) = name {
                soapycheck!("set TX gain element",
                    dev.set_gain_element(soapysdr::Direction::Tx, conf.tx_chan, name.as_bytes().to_vec(), *value));
            } else {
                soapycheck!("set TX overall gain",
                    dev.set_gain(soapysdr::Direction::Tx, conf.tx_chan, *value));
            }
        }
        */
        let mut rx = if rx_enabled {
            Some(soapycheck!("setup RX stream",
                dev.rx_stream_args(&[rx_ch], convert_args(&cli.rx_args))))
        } else {
            None
        };
        let mut tx = if tx_enabled {
            Some(soapycheck!("setup TX stream",
                dev.tx_stream_args(&[tx_ch], convert_args(&cli.tx_args))))
        } else {
            None
        };
        if let Some(rx) = &mut rx {
            soapycheck!("activate RX stream",
                rx.activate(None));
        }
        if let Some(tx) = &mut tx {
            soapycheck!("activate TX stream",
                tx.activate(None));
        }
        Ok(Self {
            rx_ch,
            tx_ch,
            dev,
            rx,
            tx,
        })
    }

    pub fn receive(&mut self, buffer: &mut [StreamType]) -> Result<soapysdr::StreamResult, soapysdr::Error> {
        if let Some(rx) = &mut self.rx {
            // TODO: implement read_exact and use that
            rx.read_ext(&mut [buffer], soapysdr::StreamFlags::default(), None, 100000)
        } else {
            Err(soapysdr::Error {
                code: soapysdr::ErrorCode::StreamError,
                message: "RX is disabled".to_string(),
            })
        }
    }

    pub fn transmit(&mut self, buffer: &[StreamType], timestamp: Option<i64>) -> Result<(), soapysdr::Error> {
        if let Some(tx) = &mut self.tx {
            tx.write_all(&[buffer], timestamp, false, 100000)
        } else {
            Err(soapysdr::Error {
                code: soapysdr::ErrorCode::StreamError,
                message: "TX is disabled".to_string(),
            })
        }
    }

    pub fn rx_sample_rate(&self) -> Result<f64, soapysdr::Error> {
        self.dev.sample_rate(soapysdr::Direction::Rx, self.rx_ch)
    }

    pub fn tx_sample_rate(&self) -> Result<f64, soapysdr::Error> {
        self.dev.sample_rate(soapysdr::Direction::Tx, self.tx_ch)
    }

    pub fn rx_center_frequency(&self) -> Result<f64, soapysdr::Error> {
        self.dev.frequency(soapysdr::Direction::Rx, self.rx_ch)
    }

    pub fn tx_center_frequency(&self) -> Result<f64, soapysdr::Error> {
        self.dev.frequency(soapysdr::Direction::Tx, self.tx_ch)
    }
}
