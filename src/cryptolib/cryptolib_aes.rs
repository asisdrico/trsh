// Copyright (c) 2021 asisdrico <asisdrico@outlook.com>
//
// Licensed under the MIT license
// <LICENSE or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use aes::cipher::stream::InvalidKeyNonceLength;
use aes::Aes128;

use ofb::cipher::{NewStreamCipher, SyncStreamCipher};
use ofb::Ofb;

use std::io;
use std::{
    io::{ErrorKind, Read, Write},
    sync::mpsc,
};

type AesOfb = Ofb<Aes128>;

const DEFAULT_BUF_SIZE: usize = 1;
const BUF_SIZE: usize = 8*1024;
pub struct Crypto {
    cipher: AesOfb,
    buffer: Vec<u8>,
}

impl Crypto {
    pub fn new(key: &[u8], iv: &[u8]) -> Result<Self, InvalidKeyNonceLength> {
        let cipher = AesOfb::new_var(key, iv)?;
        let buffer = vec![];

        Ok(Self {
            cipher: cipher,
            buffer: buffer,
        })
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub fn copy_buf<R: ?Sized, W: ?Sized>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        tx: &mpsc::Sender<u64>,
    ) -> io::Result<u64>
    where
        R: Read,
        W: Write,
    {
        let mut buffer: [u8; BUF_SIZE] = [0; BUF_SIZE];

        let mut written = 0;
        loop {
            let len = match reader.read(&mut buffer) {
                Ok(0) => return Ok(written),
                Ok(len) => len,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            self.cipher.apply_keystream(&mut buffer[..len]);
            writer.write_all(&mut buffer[..len])?;
            written += len as u64;
            tx.send(written).unwrap();
        }
    }

    pub fn copy<R: ?Sized, W: ?Sized>(&mut self, reader: &mut R, writer: &mut W) -> io::Result<u64>
    where
        R: Read,
        W: Write,
    {
        let mut buffer: [u8; DEFAULT_BUF_SIZE] = [0; DEFAULT_BUF_SIZE];

        let mut written = 0;
        loop {
            let len = match reader.read(&mut buffer) {
                Ok(0) => return Ok(written),
                Ok(len) => len,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            self.cipher.apply_keystream(&mut buffer[..len]);
            writer.write_all(&mut buffer[..len])?;
            for elem in buffer.iter_mut() { *elem = 0; };
            written += len as u64;
        }
    }
}

impl Read for Crypto {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.buffer = buf.to_vec();
        self.cipher.apply_keystream(&mut self.buffer);
        Ok(buf.len())
    }
}

impl Write for Crypto {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer = buf.to_vec();
        self.cipher.apply_keystream(&mut self.buffer);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        unimplemented!()
    }
}
