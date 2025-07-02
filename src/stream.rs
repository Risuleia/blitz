use std::{fmt::Debug, io::{self, Read, Write}, net::TcpStream};

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Plain,
    Tls
}

pub trait NoDelay {
    fn set_nodelay(&mut self, no_delay: bool) -> io::Result<()>;
}

impl NoDelay for TcpStream {
    fn set_nodelay(&mut self, no_delay: bool) -> io::Result<()> {
        TcpStream::set_nodelay(&self, no_delay)
    }
}

pub enum SimpleStream {
    Plain(TcpStream)
}

impl Debug for SimpleStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(s) => f.debug_tuple("SimpleStream::Plain").field(s).finish()
        }
    }
}

impl Read for SimpleStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf)
        }
    }
}

impl Write for SimpleStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.flush()
        }
    }
}

impl NoDelay for SimpleStream {
    fn set_nodelay(&mut self, no_delay: bool) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.set_nodelay(no_delay)
        }
    }
}