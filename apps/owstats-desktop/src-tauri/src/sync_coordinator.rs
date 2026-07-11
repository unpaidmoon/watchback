use std::sync::{Arc, Mutex};

const SYNC_IN_PROGRESS_MESSAGE: &str = "已有赛季同步正在进行，请等待完成后再试";

#[derive(Clone, Default)]
pub struct SyncCoordinator {
    running: Arc<Mutex<bool>>,
}

pub struct SyncPermit {
    running: Arc<Mutex<bool>>,
}

impl SyncCoordinator {
    pub fn try_acquire(&self) -> Result<SyncPermit, &'static str> {
        let mut running = self
            .running
            .try_lock()
            .map_err(|_| SYNC_IN_PROGRESS_MESSAGE)?;
        if *running {
            return Err(SYNC_IN_PROGRESS_MESSAGE);
        }
        *running = true;
        Ok(SyncPermit {
            running: Arc::clone(&self.running),
        })
    }
}

impl Drop for SyncPermit {
    fn drop(&mut self) {
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_sync_can_run_and_drop_releases_the_guard() {
        let coordinator = SyncCoordinator::default();
        let guard = coordinator.try_acquire().unwrap();
        assert!(matches!(
            coordinator.try_acquire(),
            Err(SYNC_IN_PROGRESS_MESSAGE)
        ));
        drop(guard);
        assert!(coordinator.try_acquire().is_ok());
    }
}
