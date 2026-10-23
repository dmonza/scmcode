//! scmCode: keep blocking work off the main thread and off the async workers
//! (docs/audit TR-16).
//!
//! Tauri 2 runs a non-`async` command on the main thread, so a slow one freezes
//! the window; an `async` command runs on a tokio worker, so blocking I/O inside
//! it starves every other async command. Commands that read files, walk a
//! folder, run git or talk to the OS keyring go through [`off_main`], which runs
//! them on the blocking pool. Writes to a child process's input (terminal,
//! debug adapter) go through a [`WriteQueue`], as VS Code's `terminalProcess.ts`
//! queues PTY writes (`_writeQueue`), so a child that stops reading can't block
//! the caller or the store lock it holds.

use std::io::Write;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Run `f` on the blocking thread pool and wait for it without blocking the
/// caller's thread. A panic in `f` comes back as an error.
pub async fn off_main<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("background task failed: {e}"))?
}

/// A dedicated writer thread for one child's input.
pub struct WriteQueue {
    tx: mpsc::Sender<Vec<u8>>,
    error: Arc<Mutex<Option<String>>>,
}

impl WriteQueue {
    /// Move `writer` to its own thread. The thread ends when the queue is
    /// dropped or a write fails.
    pub fn spawn(mut writer: Box<dyn Write + Send>) -> Self {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let error = Arc::new(Mutex::new(None));
        let thread_error = Arc::clone(&error);
        std::thread::spawn(move || {
            for chunk in rx {
                if let Err(e) = writer.write_all(&chunk).and_then(|()| writer.flush()) {
                    if let Ok(mut slot) = thread_error.lock() {
                        *slot = Some(e.to_string());
                    }
                    break;
                }
            }
        });
        Self { tx, error }
    }

    /// Queue `data` for writing. Returns at once; fails only if an earlier write
    /// failed or the writer is gone.
    pub fn send(&self, data: Vec<u8>) -> Result<(), String> {
        if let Some(e) = self.error.lock().ok().and_then(|slot| slot.clone()) {
            return Err(e);
        }
        self.tx.send(data).map_err(|_| "writer closed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Condvar;
    use std::time::{Duration, Instant};

    /// A writer that blocks until released, like a PTY whose program stopped
    /// reading.
    struct Stuck(Arc<(Mutex<bool>, Condvar)>, Arc<Mutex<Vec<u8>>>);

    impl Write for Stuck {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let (lock, cvar) = &*self.0;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = cvar.wait(released).unwrap();
            }
            self.1.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn send_returns_while_the_writer_is_blocked_and_keeps_order() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let out = Arc::new(Mutex::new(Vec::new()));
        let queue = WriteQueue::spawn(Box::new(Stuck(Arc::clone(&gate), Arc::clone(&out))));

        let start = Instant::now();
        for i in 0..100u8 {
            queue.send(vec![i; 1024]).unwrap();
        }
        assert!(start.elapsed() < Duration::from_secs(1), "send blocked");

        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        let deadline = Instant::now() + Duration::from_secs(5);
        while out.lock().unwrap().len() < 100 * 1024 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let out = out.lock().unwrap();
        assert_eq!(out.len(), 100 * 1024);
        assert!(out
            .chunks(1024)
            .enumerate()
            .all(|(i, c)| c.iter().all(|&b| b as usize == i)));
    }

    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("pipe closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_failed_write_is_reported_on_the_next_send() {
        let queue = WriteQueue::spawn(Box::new(Broken));
        queue.send(b"x".to_vec()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while queue.send(b"y".to_vec()).is_ok() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(queue.send(b"z".to_vec()).is_err());
    }

    #[test]
    fn off_main_returns_the_result_and_turns_a_panic_into_an_error() {
        let ok = tauri::async_runtime::block_on(off_main(|| Ok::<_, String>(7)));
        assert_eq!(ok, Ok(7));
        let panicked =
            tauri::async_runtime::block_on(off_main(|| -> Result<(), String> { panic!("boom") }));
        assert!(panicked.unwrap_err().contains("background task failed"));
    }
}
