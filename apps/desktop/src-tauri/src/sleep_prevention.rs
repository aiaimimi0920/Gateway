//! Execution-state requests are thread-owned; one worker applies and releases them.
use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Mutex};
use std::thread::JoinHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SleepMode {
    #[default]
    Off,
    System,
    Display,
}

impl SleepMode {
    fn flags(self) -> u32 {
        0x80000000
            | match self {
                Self::Off => 0,
                Self::System => 1,
                Self::Display => 3,
            }
    }
}

type Request = (SleepMode, mpsc::SyncSender<Result<(), String>>);
pub struct SleepPrevention {
    worker: Mutex<Option<(mpsc::SyncSender<Request>, JoinHandle<()>)>>,
}

impl Default for SleepPrevention {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
        }
    }
}

impl SleepPrevention {
    pub fn apply(&self, mode: SleepMode) -> Result<(), String> {
        if !cfg!(windows) {
            return if mode == SleepMode::Off {
                Ok(())
            } else {
                Err("Sleep prevention is supported on Windows only".into())
            };
        }
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| "Power worker lock poisoned")?;
        if worker.is_none() {
            let (sender, receiver) = mpsc::sync_channel::<Request>(1);
            let handle = std::thread::Builder::new()
                .name("gateway-power".into())
                .spawn(move || {
                    while let Ok((mode, reply)) = receiver.recv() {
                        let _ = reply.send(set_execution_state(mode));
                    }
                    // The same thread must clear its request. Thread exit also releases it in Windows.
                    let _ = set_execution_state(SleepMode::Off);
                })
                .map_err(|_| "Cannot start power worker")?;
            *worker = Some((sender, handle));
        }
        let (reply, result) = mpsc::sync_channel(1);
        worker
            .as_ref()
            .unwrap()
            .0
            .send((mode, reply))
            .map_err(|_| "Power worker stopped")?;
        result.recv().map_err(|_| "Power worker did not respond")?
    }

    pub fn stop(&self) {
        if let Ok(mut worker) = self.worker.lock() {
            if let Some((sender, handle)) = worker.take() {
                drop(sender);
                let _ = handle.join();
            }
        }
    }
}

impl Drop for SleepPrevention {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(windows)]
fn set_execution_state(mode: SleepMode) -> Result<(), String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }
    // Valid constant flags only, no pointers; all calls stay on the owned worker.
    if unsafe { SetThreadExecutionState(mode.flags()) } == 0 {
        Err("Windows rejected the sleep prevention request".into())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn set_execution_state(_: SleepMode) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn owned_worker_applies_all_modes_and_releases_on_stop() {
        let power = SleepPrevention::default();
        for mode in [SleepMode::System, SleepMode::Display, SleepMode::Off] {
            power.apply(mode).unwrap();
        }
        power.stop();
        assert!(power.worker.lock().unwrap().is_none());
        power.stop();
    }
    #[test]
    fn modes_map_to_continuous_requests_without_away_mode() {
        assert_eq!(SleepMode::Off.flags(), 0x80000000);
        assert_eq!(SleepMode::System.flags(), 0x80000001);
        assert_eq!(SleepMode::Display.flags(), 0x80000003);
        assert!(serde_json::from_str::<SleepMode>("\"invalid\"").is_err());
    }
}
