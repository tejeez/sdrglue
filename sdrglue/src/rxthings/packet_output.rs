
use crate::dsp_types::SampleCount;

enum Socket {
    Udp(std::net::UdpSocket),
    // TODO: somehow remove UnixDatagram on platforms that do not have it
    UnixDatagram(std::os::unix::net::UnixDatagram),
}

pub struct PacketOutput {
    socket: Socket,
    /// Maximum number of bytes in a packet
    max_bytes: usize,
    /// Add a header with sample count to each packet
    use_header: bool,
    /// Buffer to construct a packet
    buffer: Vec<u8>,
}

impl PacketOutput {
    pub fn new(address: &str, max_bytes: usize, use_header: bool) -> std::io::Result<Self> {
        let socket = if let Some(address) = address.strip_prefix("udp:") {
            let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
            socket.connect(address)?;
            socket.set_nonblocking(true)?;
            Socket::Udp(socket)
        } else if let Some(path) = address.strip_prefix("unix:") {
            let socket = std::os::unix::net::UnixDatagram::unbound()?;
            // Re-try connecting until the client has started and the socket exists
            loop {
                match socket.connect(path) {
                    Ok(()) => break,
                    Err(err) => {
                        tracing::debug!("Could not open output socket yet: {}", err);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                }
            }
            socket.set_nonblocking(true)?;
            Socket::UnixDatagram(socket)
        } else {
            // TODO: return better error type?
            return Err(std::io::Error::other("Address should start with udp: or unix:"));
        };

        Ok(Self {
            socket,
            max_bytes,
            use_header,
            buffer: Vec::with_capacity(max_bytes),
        })
    }

    fn add_header_if_needed(&mut self, count: SampleCount) {
        if self.buffer.len() == 0 && self.use_header {
            let header_value: u64 = 0; // placeholder, not decided yet
            self.buffer.extend_from_slice(&header_value.to_le_bytes());
            self.buffer.extend_from_slice(&count.to_le_bytes());
        }
    }

    /// Add a sample to a packet
    pub fn add_sample(&mut self, count: SampleCount, sample: &[u8]) {
        // Send and start a new packet if the sample would not fit anymore
        if self.buffer.len() + sample.len() > self.max_bytes {
            self.send();
        }
        self.add_header_if_needed(count);
        self.buffer.extend_from_slice(sample);
    }

    /// Add a sample with two elements to a packet
    pub fn add_sample2(&mut self, count: SampleCount, sample0: &[u8], sample1: &[u8]) {
        // Send and start a new packet if the sample would not fit anymore
        if self.buffer.len() + sample0.len() + sample1.len() > self.max_bytes {
            self.send();
        }
        self.add_header_if_needed(count);
        self.buffer.extend_from_slice(sample0);
        self.buffer.extend_from_slice(sample1);
    }

    /// Send the packet in the output buffer
    pub fn send(&mut self) {
        // TODO: check result and figure out what to do with each error
        let _ = match &self.socket {
            Socket::Udp(socket) =>
                socket.send(&self.buffer),
            Socket::UnixDatagram(socket) =>
                socket.send(&self.buffer),
        };

        self.buffer.clear();
    }

    pub fn send_if_full(&mut self) {
        if self.buffer.len() == self.max_bytes {
            self.send()
        }
    }
}
