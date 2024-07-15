use embedded_io::{Read, Write};
use heapless::{String, Vec};

use esp_wifi::{wifi::WifiStaDevice, wifi_interface::Socket};

use crate::SocketError;

pub fn fetch_current_front_name(
    socket: &mut Socket<'_, '_, WifiStaDevice>,
) -> Result<String<32>, SocketError> {
    socket.write(b" ")?;
    socket.flush()?;

    let mut buf = [0; 32];
    socket.read_exact(&mut buf).map_err(|err| match err {
        embedded_io::ReadExactError::UnexpectedEof => SocketError::SocketClosed,
        embedded_io::ReadExactError::Other(other) => other,
    })?;

    Ok(String::from_utf8(Vec::from_slice(&buf).unwrap()).unwrap())
}
