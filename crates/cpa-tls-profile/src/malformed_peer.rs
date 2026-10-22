//! Test-only mutation of the real initial hybrid share, preserving all unrelated TLS fields.
use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

pub struct Peer {
    pub socket: TcpStream,
    pub wrong_length: Option<bool>,
}

fn u16_at(bytes: &[u8], at: usize) -> usize {
    u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn corrupt(bytes: &mut [u8], wrong_length: bool) {
    assert_eq!(&bytes[..3], &[22, 3, 1]);
    assert_eq!(bytes[5], 1);
    let mut at = 5 + 4 + 2 + 32;
    at += 1 + bytes[at] as usize;
    at += 2 + u16_at(bytes, at);
    at += 1 + bytes[at] as usize;
    let end = at + 2 + u16_at(bytes, at);
    at += 2;
    while at < end {
        let kind = u16_at(bytes, at);
        let len = u16_at(bytes, at + 2);
        if kind == 51 {
            let share = at + 4 + 2;
            assert!(matches!(u16_at(bytes, share), 4587 | 4589));
            if wrong_length {
                let len = (u16_at(bytes, share + 2) - 1) as u16;
                bytes[share + 2..share + 4].copy_from_slice(&len.to_be_bytes());
            } else {
                // EC points in both groups must start with uncompressed-point tag 4.
                assert_eq!(bytes[share + 4], 4);
                bytes[share + 4] = 0;
            }
            return;
        }
        at += 4 + len;
    }
    panic!("generated handshake did not contain a hybrid key share");
}

impl Read for Peer {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.socket.read(bytes)
    }
}
impl Write for Peer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(wrong_length) = self.wrong_length.take() {
            let mut changed = bytes.to_vec();
            corrupt(&mut changed, wrong_length);
            self.socket.write_all(&changed)?;
            return Ok(bytes.len());
        }
        self.socket.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}
