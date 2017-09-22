use std::io::Read;
use std::io::Result;
use std::io::Write;

use sha2;
use sha2::Digest;

pub struct FilterRead<R: Read> {
    inner: R,
    hash: sha2::Sha512,
}

pub struct FilterWrite<W: Write> {
    inner: W,
    hash: sha2::Sha512,
}

impl<R: Read> Read for FilterRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let len = self.inner.read(buf)?;
        self.hash.input(&buf[0..len]);
        Ok(len)
    }
}

impl<R: Read> FilterRead<R> {
    pub fn new(inner: R) -> Self {
        FilterRead {
            inner,
            hash: sha2::Sha512::default(),
        }
    }

    #[allow(unused)]
    pub fn into_inner(self) -> R {
        self.inner
    }

    pub fn hash(&mut self) -> Vec<u8> {
        self.hash.result().into_iter().collect()
    }
}

impl<W: Write> Write for FilterWrite<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let len = self.inner.write(buf)?;
        self.hash.input(&buf[0..len]);
        Ok(len)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

impl<W: Write> FilterWrite<W> {
    pub fn new(inner: W) -> Self {
        FilterWrite {
            inner,
            hash: sha2::Sha512::default(),
        }
    }

    #[allow(unused)]
    pub fn into_inner(self) -> W {
        self.inner
    }

    pub fn hash(&mut self) -> Vec<u8> {
        self.hash.result().into_iter().collect()
    }
}
