use soapysdr;
use crate::configuration;

type StreamType = crate::ComplexSample;

struct SdrDefaults<'a> {
    /// Name used to print which SDR was detected
    pub name: &'a str,
    /// Receive sample rate
    pub rx_fs: f64,
    /// Transmit sample rate
    pub tx_fs: f64,
    /// Receive antenna
    pub rx_ant:   Option<&'a str>,
    /// Transmit antenna
    pub tx_ant:   Option<&'a str>,
    /// Receive gain(s)
    pub rx_gain: &'a[&'a str],
    /// Transmit gain(s)
    pub tx_gain: &'a[&'a str],
}


/// Default settings for LimeSDR
const SDR_DEFAULTS_LIME: SdrDefaults = SdrDefaults {
    name: "LimeSDR",
    rx_fs: 8192e3,
    tx_fs: 8192e3,
    rx_ant: Some("LNAL"),
    tx_ant: Some("BAND1"),
    rx_gain: &[
        "LNA", "20.0",
        "TIA", "10.0",
        "PGA", "10.0",
    ],
    tx_gain: &[
        "PAD",  "52.0",
        "IAMP",  "3.0",
    ],
};

/// Default settings for SXceiver
const SDR_DEFAULTS_SX: SdrDefaults = SdrDefaults {
    name: "SXceiver",
    rx_fs: 600e3,
    tx_fs: 600e3,
    rx_ant: Some("RX"),
    tx_ant: Some("TX"),
    rx_gain: &[
        "LNA", "42.0",
        "PGA", "16.0",
    ],
    tx_gain: &[
        "DAC",    "9.0",
        "MIXER", "30.0",
    ],
};

/// Default settings for RTL-SDR
const SDR_DEFAULTS_RTLSDR: SdrDefaults = SdrDefaults {
    name: "RTL-SDR",
    rx_fs: 2400e3,
    tx_fs: 2400e3,
    rx_ant: Some("RX"),
    tx_ant: None,
    rx_gain: &["40.0"],
    tx_gain: &[],
};

/// Default settings for any other SDR
const SDR_DEFAULTS: SdrDefaults = SdrDefaults {
    name: "unknown SDR device",
    rx_fs: 2048e3,
    tx_fs: 2048e3,
    rx_ant: None,
    tx_ant: None,
    rx_gain: &[],
    tx_gain: &[],
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

        let mut dev = soapycheck!("open SoapySDR device",
            soapysdr::Device::new(convert_args(&cli.sdr_device)));

        let rx_enabled = cli.sdr_rx_freq.is_some()
            && (dev.num_channels(soapysdr::Direction::Rx).unwrap_or(0) > 0);
        let tx_enabled = cli.sdr_tx_freq.is_some()
            && (dev.num_channels(soapysdr::Direction::Tx).unwrap_or(0) > 0);

        let sdr_defaults = match(
            dev.driver_key()  .unwrap_or("".to_string()).as_str(),
            dev.hardware_key().unwrap_or("".to_string()).as_str()
        ) {
            // TODO: other LimeSDR models
            //("FX3", _) => &SDR_DEFAULTS_LIME,
            (_, "LimeSDR-USB") => &SDR_DEFAULTS_LIME,

            ("sx", _) => &SDR_DEFAULTS_SX,
            (_, "sx") => &SDR_DEFAULTS_SX,

            // We could also use hardware key to use different defaults
            // for different RTL-SDR tuner chips.
            ("RTLSDR", _) => &SDR_DEFAULTS_RTLSDR,

            (_, _) => &SDR_DEFAULTS,
        };
        eprintln!("Using default settings for {}", sdr_defaults.name);

        // If only one of RX or TX sample rates is set, use the same one for both.
        // Some SDRs require both sample rates to be equal anyway.
        // If none are set, use default values.
        if rx_enabled {
            soapycheck!("set RX sample rate",
                dev.set_sample_rate(soapysdr::Direction::Rx, rx_ch,
                    cli.sdr_rx_fs.unwrap_or(cli.sdr_tx_fs.unwrap_or(sdr_defaults.rx_fs))));
        }
        if tx_enabled {
            soapycheck!("set TX sample rate",
                dev.set_sample_rate(soapysdr::Direction::Tx, tx_ch,
                    cli.sdr_tx_fs.unwrap_or(cli.sdr_rx_fs.unwrap_or(sdr_defaults.tx_fs))));
        }

        if rx_enabled {
            // If rx_enabled is true, we already know sdr_rx_freq is not None,
            // so unwrap is fine here.
            soapycheck!("set RX center frequency",
            dev.set_frequency(soapysdr::Direction::Rx, rx_ch,
                cli.sdr_rx_freq.unwrap(),
                soapysdr::Args::new()));

            if let Some(ant) =
                if let Some(ant) = &cli.sdr_rx_ant
                    { Some(ant.as_str()) } else { sdr_defaults.rx_ant }
            {
                soapycheck!("set RX antenna",
                dev.set_antenna(soapysdr::Direction::Rx, rx_ch, ant));
            }

            set_gains(&mut dev, soapysdr::Direction::Rx, rx_ch,
                &cli.sdr_rx_gain, sdr_defaults.rx_gain)?;
        }

        if tx_enabled {
            soapycheck!("set TX center frequency",
            dev.set_frequency(soapysdr::Direction::Tx, tx_ch,
                cli.sdr_tx_freq.unwrap(),
                soapysdr::Args::new()));

            if let Some(ant) =
                if let Some(ant) = &cli.sdr_tx_ant
                    { Some(ant.as_str()) } else { sdr_defaults.tx_ant }
            {
                soapycheck!("set TX antenna",
                dev.set_antenna(soapysdr::Direction::Tx, tx_ch, ant));
            }

            set_gains(&mut dev, soapysdr::Direction::Tx, tx_ch,
                &cli.sdr_tx_gain, sdr_defaults.tx_gain)?;
        }

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


/// Parse gains from command line and set them
fn set_gains(
    dev: &mut soapysdr::Device,
    direction: soapysdr::Direction,
    channel: usize,
    cli_gains: &Vec<String>,
    defaults: &[&str]
) -> Result<(), soapysdr::Error> {
    // Clap uses String but that cannot be used in default structs,
    // so we need some extra conversion here to make them the same type.
    // Maybe there would be some cleaner way to do it.
    let gains: &[String] = if cli_gains.len() > 0 {
        &cli_gains[..]
    } else {
        &defaults.iter().map(|g| g.to_string()).collect::<Vec<String>>()[..]
    };

    let element_gains = if gains.len() % 2 == 1 {
        // Odd number: use first one to set overall gain,
        // others as pairs to set gain elements if given.
        match gains[0].parse::<f64>() {
            Ok(gain) => {
                soapycheck!("set overall gain",
                dev.set_gain(direction, channel, gain));
            }
            Err(err) => {
                eprintln!("Error parsing overall gain value {}: {}", gains[0], err);
            }
        }
        &gains[1..]
    } else {
        gains
    };

    for element in element_gains.chunks_exact(2) {
        match element[1].parse::<f64>() {
            Ok(gain) => {
                soapycheck!("set element gain",
                dev.set_gain_element(direction, channel, element[0].as_str(), gain));
            }
            Err(err) => {
                eprintln!("Error parsing element gain value {}: {}", element[1], err);
            }
        }
    }

    Ok(())
}
