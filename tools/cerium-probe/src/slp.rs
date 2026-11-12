//! Server List Ping client: handshake, status request, then ping/pong.
//!
//! The packet layout follows vanilla 26.3. The handshake intent `1` selects the status
//! protocol. There, the status request and response are packet `0x00`, and the ping and
//! pong are packet `0x01`, each carrying a big-endian `i64`.

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::wire::{invalid, read_packet, read_string, write_packet, write_string, write_varint};

const HANDSHAKE_ID: i32 = 0x00;
const INTENT_STATUS: i32 = 1;
const STATUS_REQUEST_ID: i32 = 0x00;
const STATUS_RESPONSE_ID: i32 = 0x00;
const PING_REQUEST_ID: i32 = 0x01;
const PONG_RESPONSE_ID: i32 = 0x01;

/// Fixed ping payload: deterministic, so runs are byte-for-byte reproducible.
const PING_PAYLOAD: i64 = 0x0123_4567_89AB_CDEF;

/// A successful status exchange.
#[derive(Debug, Clone)]
pub(crate) struct StatusReply {
    /// The raw status JSON sent by the server.
    pub(crate) json: String,
    /// Round-trip time of the ping/pong exchange.
    pub(crate) ping: Duration,
}

/// Connects to `host:port` and performs one complete status exchange.
pub(crate) fn query(
    host: &str,
    port: u16,
    protocol: i32,
    timeout: Duration,
) -> io::Result<StatusReply> {
    let addr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| invalid("host did not resolve to any address"))?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.set_nodelay(true)?;

    let mut handshake = Vec::new();
    write_varint(&mut handshake, protocol);
    write_string(&mut handshake, host)?;
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, INTENT_STATUS);
    write_packet(&mut stream, HANDSHAKE_ID, &handshake)?;
    write_packet(&mut stream, STATUS_REQUEST_ID, &[])?;

    let (id, body) = read_packet(&mut stream)?;
    expect_id(id, STATUS_RESPONSE_ID)?;
    let json = read_string(&mut body.as_slice())?;

    let sent = Instant::now();
    write_packet(&mut stream, PING_REQUEST_ID, &PING_PAYLOAD.to_be_bytes())?;
    let (id, body) = read_packet(&mut stream)?;
    let ping = sent.elapsed();
    expect_id(id, PONG_RESPONSE_ID)?;
    if body[..] != PING_PAYLOAD.to_be_bytes()[..] {
        return Err(invalid("pong payload does not match the ping"));
    }

    Ok(StatusReply { json, ping })
}

fn expect_id(actual: i32, expected: i32) -> io::Result<()> {
    if actual == expected {
        Ok(())
    } else {
        let message = format!("expected packet 0x{expected:02X}, got 0x{actual:02X}");
        Err(invalid(&message))
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    use crate::wire::read_varint;

    const STATUS_JSON: &str = r#"{"version":{"name":"26.3","protocol":777}}"#;

    /// A tiny fake server that checks the client's bytes and answers like vanilla.
    fn serve_once(listener: &TcpListener) -> io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        let port = listener.local_addr()?.port();

        let (id, body) = read_packet(&mut stream)?;
        assert_eq!(id, HANDSHAKE_ID);
        let mut cursor = body.as_slice();
        assert_eq!(read_varint(&mut cursor)?, 777);
        assert_eq!(read_string(&mut cursor)?, "127.0.0.1");
        assert_eq!(cursor[..2], port.to_be_bytes()[..]);
        cursor = &cursor[2..];
        assert_eq!(read_varint(&mut cursor)?, INTENT_STATUS);
        assert!(cursor.is_empty());

        let (id, body) = read_packet(&mut stream)?;
        assert_eq!((id, body.len()), (STATUS_REQUEST_ID, 0));
        let mut response = Vec::new();
        write_string(&mut response, STATUS_JSON)?;
        write_packet(&mut stream, STATUS_RESPONSE_ID, &response)?;

        let (id, body) = read_packet(&mut stream)?;
        assert_eq!(id, PING_REQUEST_ID);
        assert_eq!(body[..], PING_PAYLOAD.to_be_bytes()[..]);
        write_packet(&mut stream, PONG_RESPONSE_ID, &body)
    }

    #[test]
    fn status_exchange_against_fake_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || serve_once(&listener));

        let timeout = Duration::from_secs(10);
        let reply = query("127.0.0.1", port, 777, timeout).unwrap();
        server.join().unwrap().unwrap();
        assert_eq!(reply.json, STATUS_JSON);
    }

    #[test]
    fn wrong_packet_id_is_an_error() {
        assert!(expect_id(0x01, 0x00).is_err());
        assert!(expect_id(0x00, 0x00).is_ok());
    }
}
