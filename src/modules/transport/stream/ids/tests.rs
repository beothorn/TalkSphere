//! Sequential allocation, historical classification and integer-boundary tests.
use super::*;

// Both TCP roles allocate without gaps and retain refused IDs in their watermarks.
#[test]
fn roles_and_consumed_history() {
    for (role, local, remote) in [
        (ConnectionRole::Initiator, 1, 2),
        (ConnectionRole::Acceptor, 2, 1),
    ] {
        let mut ids = StreamIds::new(role);
        assert!(!ids.was_created(0));
        assert!(!ids.was_created(local));
        assert_eq!(ids.greatest_local(), 0);
        assert_eq!(ids.greatest_remote(), 0);
        for offset in [0, 2, 4] {
            assert_eq!(ids.allocate(), Ok(local + offset));
            assert_eq!(ids.accept(remote + offset), Ok(()));
            assert!(ids.was_created(local + offset));
            assert!(ids.was_created(remote + offset));
        }
        assert!(!ids.was_created(local + 6));
        assert!(!ids.was_created(remote + 6));
        assert_eq!(ids.greatest_local(), local + 4);
        assert_eq!(ids.greatest_remote(), remote + 4);
        assert_eq!(ids.accept(remote), Err(IdError::Unexpected));
    }
}

// Wrong parity, zero and gaps cannot consume the expected ID or allow reuse.
#[test]
fn invalid_remote_ids_leave_expected_unchanged() {
    let mut ids = StreamIds::new(ConnectionRole::Acceptor);
    for id in [0, 2, 3, u32::MAX] {
        assert_eq!(ids.accept(id), Err(IdError::Unexpected));
    }
    assert_eq!(ids.accept(1), Ok(()));
    assert_eq!(ids.accept(1), Err(IdError::Unexpected));
    assert_eq!(ids.accept(3), Ok(()));
}

// Boundary fixtures skip billions of allocations but exercise the actual checked increment.
#[test]
fn both_parities_exhaust_without_wrapping() {
    for (role, last) in [
        (ConnectionRole::Initiator, u32::MAX),
        (ConnectionRole::Acceptor, u32::MAX - 1),
    ] {
        let mut ids = StreamIds::new(role);
        ids.next_local = Some(last);
        assert_eq!(ids.allocate(), Ok(last));
        assert_eq!(ids.allocate(), Err(IdError::Exhausted));
        assert_eq!(ids.greatest_local(), last);
        let remote_last = if last == u32::MAX { last - 1 } else { u32::MAX };
        ids.next_remote = Some(remote_last);
        assert_eq!(ids.accept(remote_last), Ok(()));
        assert_eq!(ids.accept(remote_last), Err(IdError::Unexpected));
        assert_eq!(
            ids.accept(if remote_last % 2 == 1 { 1 } else { 2 }),
            Err(IdError::Unexpected)
        );
    }
}

// A shared connection owner serializes concurrent callers into one exact sequence.
#[test]
fn serialized_concurrent_allocation_has_no_duplicates_or_gaps() {
    let ids = std::sync::Mutex::new(StreamIds::new(ConnectionRole::Initiator));
    let mut allocated = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| ids.lock().unwrap().allocate().unwrap()))
            .collect();
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>()
    });
    allocated.sort_unstable();
    assert_eq!(allocated, vec![1, 3, 5, 7, 9, 11, 13, 15]);
}
