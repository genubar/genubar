use std::ffi::OsStr;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessTimeoutOutcome {
    Completed {
        success: bool,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    TimedOut,
}

/// Runs `program` with `args`, feeding `stdin_bytes` on a worker thread while concurrently
/// draining stdout and stderr so large payloads cannot deadlock OS pipes, and kills the child
/// if it exceeds `timeout_ms`.
pub fn run_command_with_stdin_timeout<S, I, A>(
    program: S,
    args: I,
    stdin_bytes: &[u8],
    timeout_ms: u64,
) -> Result<ProcessTimeoutOutcome, std::io::Error>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdin_pipe = child.stdin.take();
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(10));

    let (status_opt, stdout_buf, stderr_buf) = std::thread::scope(|scope| {
        let stdin_handle = if stdin_bytes.is_empty() {
            drop(stdin_pipe);
            None
        } else {
            Some(scope.spawn(move || {
                if let Some(mut pipe) = stdin_pipe {
                    let _ = pipe.write_all(stdin_bytes);
                }
            }))
        };

        let out_handle = scope.spawn(move || {
            let mut buf = Vec::new();
            if let Some(ref mut pipe) = stdout_pipe {
                let _ = pipe.read_to_end(&mut buf);
            }
            buf
        });
        let err_handle = scope.spawn(move || {
            let mut buf = Vec::new();
            if let Some(ref mut pipe) = stderr_pipe {
                let _ = pipe.read_to_end(&mut buf);
            }
            buf
        });

        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        break None;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };

        if let Some(handle) = stdin_handle {
            let _ = handle.join();
        }
        let out_bytes = out_handle.join().unwrap_or_default();
        let err_bytes = err_handle.join().unwrap_or_default();
        (status, out_bytes, err_bytes)
    });

    Ok(match status_opt {
        Some(status) => ProcessTimeoutOutcome::Completed {
            success: status.success(),
            stdout: stdout_buf,
            stderr: stderr_buf,
        },
        None => ProcessTimeoutOutcome::TimedOut,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_stdin_and_stdout_complete_without_pipe_deadlock() {
        let payload = vec![b'x'; 256 * 1024];
        let outcome = run_command_with_stdin_timeout("cat", Vec::<&str>::new(), &payload, 3000);
        match outcome {
            Ok(ProcessTimeoutOutcome::Completed {
                success,
                stdout,
                stderr,
            }) => {
                assert!(success);
                assert!(stderr.is_empty());
                assert_eq!(stdout.len(), payload.len());
                assert_eq!(stdout, payload);
            }
            other => panic!("Unexpected outcome from cat: {other:?}"),
        }
    }

    #[test]
    fn slow_command_is_killed_on_timeout() {
        let started = Instant::now();
        let outcome = run_command_with_stdin_timeout("sleep", ["3"], b"", 80);
        assert!(matches!(outcome, Ok(ProcessTimeoutOutcome::TimedOut)));
        assert!(started.elapsed() < Duration::from_millis(1200));
    }
}
