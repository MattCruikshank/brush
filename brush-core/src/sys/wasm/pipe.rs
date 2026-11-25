//! In-memory pipes (Wasm has no OS pipes).
//!
//! A Wasm shell runs on one thread, and brush's I/O is synchronous, so a read can't wait for a
//! writer. Pipeline stages run one after another (each is awaited before the next starts), so a
//! stage's writer has finished and closed the pipe before the next stage reads it. Reading an
//! empty pipe whose writer is still open is an error (`WouldBlock`), never a hang. Async readers
//! ([`PipeReader::poll_readable`], command substitution) do wait, by registering a waker. A pipe
//! holds at most [`CAPACITY`] bytes; writing more fails.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

/// Most bytes a pipe holds.
pub const CAPACITY: usize = 1 << 20;

#[derive(Default)]
struct Shared {
    buf: VecDeque<u8>,
    readers: usize,
    writers: usize,
    /// The reader waiting for data (or the end).
    waker: Option<Waker>,
}

fn lock(s: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

fn wake(s: &mut Shared) {
    if let Some(w) = s.waker.take() {
        w.wake();
    }
}

/// Creates a pipe.
pub fn pipe() -> io::Result<(PipeReader, PipeWriter)> {
    let shared = Arc::new(Mutex::new(Shared {
        readers: 1,
        writers: 1,
        ..Shared::default()
    }));
    Ok((PipeReader(shared.clone()), PipeWriter(shared)))
}

/// The read end of a pipe.
pub struct PipeReader(Arc<Mutex<Shared>>);

/// The write end of a pipe; the pipe ends (reads return 0) when every write end is dropped.
pub struct PipeWriter(Arc<Mutex<Shared>>);

impl PipeReader {
    /// Another handle to the read end.
    pub fn try_clone(&self) -> io::Result<Self> {
        lock(&self.0).readers += 1;
        Ok(Self(self.0.clone()))
    }

    /// Ready when a read won't fail with `WouldBlock`: data is buffered or the pipe has ended.
    pub fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<()> {
        let mut s = lock(&self.0);
        if !s.buf.is_empty() || s.writers == 0 {
            return Poll::Ready(());
        }
        s.waker = Some(cx.waker().clone());
        Poll::Pending
    }

    /// Reads everything until the pipe ends, waiting for writers.
    pub async fn read_to_end_async(&self) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        std::future::poll_fn(|cx| {
            let mut s = lock(&self.0);
            out.extend(s.buf.drain(..));
            if s.writers == 0 {
                return Poll::Ready(());
            }
            s.waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await;
        Ok(out)
    }
}

impl PipeWriter {
    /// Another handle to the write end.
    pub fn try_clone(&self) -> io::Result<Self> {
        lock(&self.0).writers += 1;
        Ok(Self(self.0.clone()))
    }

    /// Whether a reader is waiting for data on this pipe (it registered a waker and nothing has
    /// been written since).
    pub fn reader_waiting(&self) -> bool {
        let s = lock(&self.0);
        s.waker.is_some() && s.buf.is_empty()
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        lock(&self.0).readers -= 1;
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        let mut s = lock(&self.0);
        s.writers -= 1;
        if s.writers == 0 {
            wake(&mut s);
        }
    }
}

impl io::Read for PipeReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut s = lock(&self.0);
        if s.buf.is_empty() {
            if s.writers == 0 || out.is_empty() {
                return Ok(0);
            }
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "pipe is empty but its writer is still running (pipeline stages run one after another here)",
            ));
        }
        let n = out.len().min(s.buf.len());
        for (o, b) in out.iter_mut().zip(s.buf.drain(..n)) {
            *o = b;
        }
        Ok(n)
    }
}

impl io::Write for PipeWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut s = lock(&self.0);
        if s.readers == 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let room = CAPACITY - s.buf.len();
        if room == 0 && !data.is_empty() {
            return Err(io::Error::other(format!("pipe is full ({CAPACITY} bytes)")));
        }
        let n = data.len().min(room);
        s.buf.extend(&data[..n]);
        wake(&mut s);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl io::Write for PipeReader {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("can't write to the read end of a pipe"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl io::Read for PipeWriter {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("can't read from the write end of a pipe"))
    }
}

impl crate::openfiles::Stream for PipeReader {
    fn clone_box(&self) -> Box<dyn crate::openfiles::Stream> {
        Box::new(self.try_clone().expect("infallible"))
    }
    fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<()> {
        Self::poll_readable(self, cx)
    }
}

impl crate::openfiles::Stream for PipeWriter {
    fn clone_box(&self) -> Box<dyn crate::openfiles::Stream> {
        Box::new(self.try_clone().expect("infallible"))
    }
}

impl From<PipeReader> for crate::openfiles::OpenFile {
    fn from(r: PipeReader) -> Self {
        Self::Stream(Box::new(r))
    }
}

impl From<PipeWriter> for crate::openfiles::OpenFile {
    fn from(w: PipeWriter) -> Self {
        Self::Stream(Box::new(w))
    }
}
