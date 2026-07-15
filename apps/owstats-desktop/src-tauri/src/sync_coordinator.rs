use std::sync::{Arc, Mutex};

const SYNC_IN_PROGRESS_MESSAGE: &str = "账号操作或赛季同步正在进行，请等待完成后再试";

#[derive(Clone, Default)]
pub struct SyncCoordinator {
    running: Arc<Mutex<bool>>,
}

pub struct OperationPermit {
    running: Arc<Mutex<bool>>,
}

impl SyncCoordinator {
    pub fn try_acquire_account_operation(&self) -> Result<OperationPermit, &'static str> {
        self.try_acquire()
    }

    #[allow(dead_code)]
    pub fn try_acquire_sync(&self) -> Result<OperationPermit, &'static str> {
        self.try_acquire()
    }

    fn try_acquire(&self) -> Result<OperationPermit, &'static str> {
        let mut running = self
            .running
            .try_lock()
            .map_err(|_| SYNC_IN_PROGRESS_MESSAGE)?;
        if *running {
            return Err(SYNC_IN_PROGRESS_MESSAGE);
        }
        *running = true;
        Ok(OperationPermit {
            running: Arc::clone(&self.running),
        })
    }
}

impl Drop for OperationPermit {
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
    fn account_operation_blocks_sync_until_permit_is_dropped() {
        let coordinator = SyncCoordinator::default();
        let guard = coordinator.try_acquire_account_operation().unwrap();
        assert!(matches!(
            coordinator.try_acquire_sync(),
            Err(SYNC_IN_PROGRESS_MESSAGE)
        ));
        drop(guard);
        assert!(coordinator.try_acquire_sync().is_ok());
    }

    #[test]
    fn sync_blocks_account_operation_until_permit_is_dropped() {
        let coordinator = SyncCoordinator::default();
        let guard = coordinator.try_acquire_sync().unwrap();
        assert!(matches!(
            coordinator.try_acquire_account_operation(),
            Err(SYNC_IN_PROGRESS_MESSAGE)
        ));
        drop(guard);
        assert!(coordinator.try_acquire_account_operation().is_ok());
    }
}
