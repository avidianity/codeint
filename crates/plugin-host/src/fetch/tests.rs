use std::io::{self, Read, Write};

use super::{BoundedCopy, BoundedCopyError, copy_bounded};

enum ReadStep {
    Chunk(&'static [u8]),
    Interrupted,
    Error(io::ErrorKind),
}

struct ScriptedReader {
    steps: std::collections::VecDeque<ReadStep>,
}

impl Read for ScriptedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.steps.pop_front() {
            None => Ok(0),
            Some(ReadStep::Interrupted) => Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "signal interrupted read",
            )),
            Some(ReadStep::Error(kind)) => Err(io::Error::new(kind, "synthetic read failure")),
            Some(ReadStep::Chunk(bytes)) => {
                let count = bytes.len().min(buf.len());
                buf[..count].copy_from_slice(&bytes[..count]);
                Ok(count)
            }
        }
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "synthetic write failure",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn copy_bounded_retries_eintr_between_chunks() {
    let mut reader = ScriptedReader {
        steps: std::collections::VecDeque::from([
            ReadStep::Interrupted,
            ReadStep::Chunk(b"hello "),
            ReadStep::Interrupted,
            ReadStep::Chunk(b"world"),
        ]),
    };
    let mut out = Vec::new();
    let outcome = copy_bounded(&mut reader, &mut out, 64).expect("EINTR must be retried");
    assert!(matches!(outcome, BoundedCopy::Completed));
    assert_eq!(out, b"hello world");
}

#[test]
fn copy_bounded_distinguishes_read_and_write_errors() {
    let mut failing_reader = ScriptedReader {
        steps: std::collections::VecDeque::from([ReadStep::Error(io::ErrorKind::BrokenPipe)]),
    };
    let mut out = Vec::new();
    let read_err = copy_bounded(&mut failing_reader, &mut out, 64)
        .expect_err("non-EINTR read error must fail");
    assert!(
        matches!(read_err, BoundedCopyError::Read(error) if error.kind() == io::ErrorKind::BrokenPipe)
    );

    let mut reader = ScriptedReader {
        steps: std::collections::VecDeque::from([ReadStep::Chunk(b"payload")]),
    };
    let write_err =
        copy_bounded(&mut reader, &mut FailingWriter, 64).expect_err("write error must fail");
    assert!(
        matches!(write_err, BoundedCopyError::Write(error) if error.kind() == io::ErrorKind::PermissionDenied)
    );
}
