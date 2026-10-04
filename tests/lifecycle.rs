//! Owning inferred closures must release captures without a reference cycle.

use rec_closure::rec_closure;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Probe(Arc<AtomicUsize>);

impl Probe {
    fn value(&self) -> usize {
        7
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[rec_closure]
fn inferred(probe: Probe) -> impl Fn(usize) -> usize + Clone {
    let f = move |n| if n == 0 { probe.value() } else { f(n - 1) };
    f
}

#[rec_closure(sync)]
fn synchronized(probe: Probe) -> impl Fn(usize) -> usize + Clone + Send + Sync {
    let f = move |n| if n == 0 { probe.value() } else { f(n - 1) };
    f
}

#[test]
fn last_inferred_owner_releases_the_capture() {
    let drops = Arc::new(AtomicUsize::new(0));
    let f = inferred(Probe(drops.clone()));
    let other = f.clone();
    assert_eq!(f(3), 7);
    drop(f);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(other(2), 7);
    drop(other);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn concurrent_owners_release_the_capture() {
    let drops = Arc::new(AtomicUsize::new(0));
    let f = synchronized(Probe(drops.clone()));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let f = f.clone();
            std::thread::spawn(move || f(5))
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), 7);
    }
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(f);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
