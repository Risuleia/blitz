//! A buffer for reading data from the network.
//!
//! The `ReadBuffer` is a buffer of bytes similar to a first-in, first-out queue.
//! It is filled by reading from a stream supporting `Read` and is then
//! accessible as a cursor for reading bytes.

use bytes::Buf;
use std::io::{Cursor, Read, Result as IoResult};

/// A FIFO buffer for reading packets from the network.
#[derive(Debug)]
pub struct ReadBuffer<const CHUNK_SIZE: usize> {
    storage: Cursor<Vec<u8>>,
    chunk: Box<[u8; CHUNK_SIZE]>,
}

impl<const CHUNK_SIZE: usize> ReadBuffer<CHUNK_SIZE> {
    /// Initializes an empty input buffer
    pub fn new() -> Self {
        Self::with_capacity(CHUNK_SIZE)
    }

    /// Initalizes an empty input buffer with a given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: Cursor::new(Vec::with_capacity(capacity)),
            chunk: Box::new([0; CHUNK_SIZE]),
        }
    }

    /// Reads the next portion of the data from the given input stream
    pub fn read_from<S: Read>(&mut self, source: &mut S) -> IoResult<usize> {
        self.clean();

        let read_size = source.read(&mut *self.chunk)?;
        self.storage.get_mut().extend_from_slice(&self.chunk[..read_size]);

        Ok(read_size)
    }

    /// Cleans up the parts of the vector that has already been ready by the cursor
    fn clean(&mut self) {
        let pos = self.storage.position() as usize;
        self.storage.get_mut().drain(..pos);
        self.storage.set_position(0);
    }

    /// Consumes the `ReadBuffer` and gets the internal data storage
    pub fn into_vec(mut self) -> Vec<u8> {
        self.clean();
        self.storage.into_inner()
    }

    /// Gets a cursor to the data storage
    pub fn as_cursor(&self) -> &Cursor<Vec<u8>> {
        &self.storage
    }

    /// Gets a cursor to the mutable data storage
    pub fn as_cursor_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.storage
    }
}

impl<const CHUNK_SIZE: usize> Buf for ReadBuffer<CHUNK_SIZE> {
    fn remaining(&self) -> usize {
        self.storage.get_ref().len() - self.storage.position() as usize
    }

    fn chunk(&self) -> &[u8] {
        let pos = self.storage.position() as usize;
        &self.storage.get_ref()[pos..]
    }

    fn advance(&mut self, cnt: usize) {
        let new_position =
            (self.storage.position() + cnt as u64).min(self.storage.get_ref().len() as u64);
        self.storage.set_position(new_position);
    }
}

impl<const CHUNK_SIZE: usize> Default for ReadBuffer<CHUNK_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}
