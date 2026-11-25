//! Async pipe reading on Wasm (in-memory pipes; no threads for `spawn_blocking`).

use std::io;

pub(crate) struct AsyncPipeReader {
    inner: Option<super::pipe::PipeReader>,
}

impl AsyncPipeReader {
    pub(crate) fn new(reader: super::pipe::PipeReader) -> io::Result<Self> {
        Ok(Self {
            inner: Some(reader),
        })
    }

    pub(crate) async fn read_to_string(&mut self) -> io::Result<String> {
        let Some(reader) = self.inner.take() else {
            return Ok(String::new());
        };
        let bytes = reader.read_to_end_async().await?;
        String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
