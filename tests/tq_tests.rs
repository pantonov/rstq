use rstq::Tq;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::time::{self, Duration};

async fn run_blocking<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.expect("blocking task panicked")
}

#[tokio::test]
async fn new_and_with_capacity_start_empty() {
    let tq = Tq::new(Duration::from_millis(10), |_k: i32, _v: i32| {});
    run_blocking({
        let tq = tq.clone();
        move || {
            assert!(tq.is_empty());
            assert_eq!(tq.len(), 0);
            assert!(!tq.contains_key(&1));
            assert_eq!(tq.get(&1), None);
            assert_eq!(tq.remove(&1), None);
        }
    })
    .await;

    let tq2 = Tq::with_capacity(Duration::from_millis(10), |_k: i32, _v: i32| {}, 16);
    run_blocking({
        let tq2 = tq2.clone();
        move || {
            assert!(tq2.is_empty());
            assert_eq!(tq2.len(), 0);
        }
    })
    .await;
}

#[tokio::test]
async fn reserve_does_not_panic() {
    let tq = Tq::new(Duration::from_millis(10), |_k: i32, _v: i32| {});
    run_blocking({
        let tq = tq.clone();
        move || {
            tq.reserve(8);
            assert_eq!(tq.len(), 0);
        }
    })
    .await;
}

#[tokio::test]
async fn expiration_fires_and_loop_restarts() {
    let expired = Arc::new(StdMutex::new(Vec::<(i32, i32)>::new()));
    let expired_clone = expired.clone();
    let tq = Tq::new(Duration::from_millis(5), move |k: i32, v: i32| {
        expired_clone.lock().unwrap().push((k, v));
    });

    run_blocking({
        let tq = tq.clone();
        move || tq.push(1, 10)
    })
    .await;
    time::sleep(Duration::from_millis(20)).await;
    assert_eq!(expired.lock().unwrap().as_slice(), &[(1, 10)]);

    run_blocking({
        let tq = tq.clone();
        move || tq.push(2, 20)
    })
    .await;
    time::sleep(Duration::from_millis(20)).await;
    assert_eq!(expired.lock().unwrap().as_slice(), &[(1, 10), (2, 20)]);

    run_blocking({
        let tq = tq.clone();
        move || {
            assert!(!tq.contains_key(&1));
            assert_eq!(tq.get(&1), None);
            assert_eq!(tq.remove(&1), None);
        }
    })
    .await;
}
