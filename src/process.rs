use crate::model::display_text;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use std::{
    fmt,
    io::{self, Read},
    os::fd::AsFd,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum RunError {
    Io(io::Error),
    TimedOut,
    Cancelled,
    OutputLimit,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(
                f,
                "subprocess I/O error ({:?}): {}",
                error.kind(),
                display_text(&error.to_string())
            ),
            Self::TimedOut => f.write_str("subprocess timed out"),
            Self::Cancelled => f.write_str("subprocess cancelled"),
            Self::OutputLimit => f.write_str("subprocess output limit exceeded"),
        }
    }
}
impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
impl From<io::Error> for RunError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
pub struct CommandOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub trait Runner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        cancelled: &AtomicBool,
    ) -> Result<CommandOutput, RunError>;
}

pub struct ProcessRunner {
    pub timeout: Duration,
    /// Combined stdout/stderr limit. Exceeding it discards the partial result.
    pub max_output_bytes: usize,
}

impl Default for ProcessRunner {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_output_bytes: 4 * 1024 * 1024,
        }
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        // Reap on every return path, including pipe setup/read failures.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn nonblocking(pipe: &impl AsFd) -> io::Result<()> {
    let flags = fcntl_getfl(pipe)?;
    fcntl_setfl(pipe, flags | OFlags::NONBLOCK)?;
    Ok(())
}

fn read_chunk(
    pipe: &mut impl Read,
    output: &mut Vec<u8>,
    remaining: usize,
) -> Result<(bool, bool), RunError> {
    let mut buffer = [0; 8192];
    let limit = buffer.len().min(remaining.saturating_add(1));
    match pipe.read(&mut buffer[..limit]) {
        Ok(0) => Ok((true, false)),
        Ok(n) if n > remaining => Err(RunError::OutputLimit),
        Ok(n) => {
            output.extend_from_slice(&buffer[..n]);
            Ok((false, true))
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok((false, false))
        }
        Err(error) => Err(error.into()),
    }
}

impl Runner for ProcessRunner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        cancelled: &AtomicBool,
    ) -> Result<CommandOutput, RunError> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(RunError::Cancelled);
        }
        let start = Instant::now();
        let mut child = ChildGuard(
            Command::new(program)
                .args(args)
                .env("LC_ALL", "C")
                .env("SYSTEMD_COLORS", "0")
                .env("SYSTEMD_URLIFY", "0")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        );
        let mut stdout = child
            .0
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing stdout pipe"))?;
        let mut stderr = child
            .0
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing stderr pipe"))?;
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let (mut out_done, mut err_done) = (false, false);
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Err(RunError::Cancelled);
            }
            if start.elapsed() >= self.timeout {
                return Err(RunError::TimedOut);
            }
            let mut progress = false;
            if !out_done {
                let remaining = self.max_output_bytes - out.len() - err.len();
                let (done, read) = read_chunk(&mut stdout, &mut out, remaining)?;
                out_done = done;
                progress |= read;
            }
            if !err_done {
                let remaining = self.max_output_bytes - out.len() - err.len();
                let (done, read) = read_chunk(&mut stderr, &mut err, remaining)?;
                err_done = done;
                progress |= read;
            }
            if let Some(status) = child.0.try_wait()?
                && out_done
                && err_done
            {
                return Ok(CommandOutput {
                    success: status.success(),
                    code: status.code(),
                    stdout: out,
                    stderr: err,
                });
            }
            // Nonblocking pipes keep timeout/cancellation effective even when a
            // descendant retains a pipe after the direct child has exited.
            if !progress {
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

impl<R: Runner + ?Sized> Runner for &R {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        cancelled: &AtomicBool,
    ) -> Result<CommandOutput, RunError> {
        (**self).run(program, args, cancelled)
    }
}
