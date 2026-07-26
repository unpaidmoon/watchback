#[path = "../src/sync_coordinator.rs"]
mod sync_coordinator;

use std::sync::{Arc, Barrier};
use std::thread;
use sync_coordinator::{SyncCoordinator, QUERY_BLOCKS_SYNC_MESSAGE, SYNC_BLOCKS_QUERY_MESSAGE};

#[test]
fn foreground_sync_rejects_query_and_releases_cleanly() {
    let coordinator = SyncCoordinator::default();
    {
        let _sync = coordinator.try_acquire_sync().unwrap();
        assert!(matches!(
            coordinator.try_acquire_query_session(),
            Err(SYNC_BLOCKS_QUERY_MESSAGE)
        ));
    }
    assert!(coordinator.try_acquire_query_session().is_ok());
}

#[test]
fn background_registration_is_atomic_with_query_acquisition() {
    for _ in 0..100 {
        let coordinator = Arc::new(SyncCoordinator::default());
        let barrier = Arc::new(Barrier::new(3));
        let sync_thread = {
            let coordinator = Arc::clone(&coordinator);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                coordinator.try_register_background_sync().ok()
            })
        };
        let query_thread = {
            let coordinator = Arc::clone(&coordinator);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                coordinator.try_acquire_query_session().ok()
            })
        };
        barrier.wait();
        let background = sync_thread.join().unwrap();
        let query = query_thread.join().unwrap();
        assert_ne!(background.is_some(), query.is_some());
    }
}

#[test]
fn query_session_rejects_every_sync_entry_point_and_drop_restores_access() {
    let coordinator = SyncCoordinator::default();
    let query = coordinator.try_acquire_query_session().unwrap();
    assert!(matches!(
        coordinator.try_acquire_sync(),
        Err(QUERY_BLOCKS_SYNC_MESSAGE)
    ));
    assert!(matches!(
        coordinator.try_register_background_sync(),
        Err(QUERY_BLOCKS_SYNC_MESSAGE)
    ));
    drop(query);
    assert!(coordinator.try_acquire_sync().is_ok());
}

#[test]
fn queued_and_running_background_lifetimes_reject_query() {
    let coordinator = SyncCoordinator::default();
    let mut background = coordinator.try_register_background_sync().unwrap();
    assert!(matches!(
        coordinator.try_acquire_query_session(),
        Err(SYNC_BLOCKS_QUERY_MESSAGE)
    ));
    background.mark_running();
    assert!(matches!(
        coordinator.try_acquire_query_session(),
        Err(SYNC_BLOCKS_QUERY_MESSAGE)
    ));
    drop(background);
    assert!(coordinator.try_acquire_query_session().is_ok());
}

#[test]
fn panic_unwind_does_not_leak_query_or_sync_permits() {
    let coordinator = SyncCoordinator::default();
    let query_result = std::panic::catch_unwind({
        let coordinator = coordinator.clone();
        move || {
            let _query = coordinator.try_acquire_query_session().unwrap();
            panic!("simulated query failure");
        }
    });
    assert!(query_result.is_err());
    let sync = coordinator.try_acquire_sync().unwrap();
    drop(sync);

    let sync_result = std::panic::catch_unwind({
        let coordinator = coordinator.clone();
        move || {
            let _sync = coordinator.try_acquire_sync().unwrap();
            panic!("simulated sync failure");
        }
    });
    assert!(sync_result.is_err());
    assert!(coordinator.try_acquire_query_session().is_ok());
}
