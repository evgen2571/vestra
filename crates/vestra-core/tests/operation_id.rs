use std::{collections::HashSet, thread};

use vestra_core::OperationId;

#[test]
fn operation_ids_are_copyable_serializable_and_unique_across_threads() {
    let first = OperationId::new();
    let copied = first;
    assert_eq!(first, copied);
    assert_eq!(
        first.to_string(),
        serde_json::to_string(&first).expect("serializes")
    );

    let mut handles = Vec::new();
    for _ in 0..8 {
        handles.push(thread::spawn(|| {
            (0..128).map(|_| OperationId::new()).collect::<Vec<_>>()
        }));
    }

    let ids = handles
        .into_iter()
        .flat_map(|handle| handle.join().expect("generation thread succeeds"))
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), 1024);
    assert!(!ids.contains(&first));
}
