use ordhash::OrdHash;
use std::{hash::Hash, time};
use std::sync::{Arc, atomic::Ordering, atomic::AtomicBool};
use tokio::sync::Mutex;

#[derive(Clone)]
struct ExpValue<V> {
    value: V,
    expireat: time::SystemTime,
}

#[derive(Clone)]
pub struct Tq<K, V, F> {
    inner:   Arc<Mutex<OrdHash<K, ExpValue<V>>>>,
    expfunc: F,
    timeout: time::Duration,
    running: Arc<AtomicBool>,
}

impl<K: Eq + Hash + Clone + Send, V: Clone + Send, F: Fn(K, V) + Clone + Send> Tq<K, V, F> {
    pub fn with_capacity(timeout: time::Duration, expfunc: F, cap: usize) -> Self
    {
        Tq {
            inner:   Arc::new(Mutex::new(OrdHash::with_capacity(cap))),
            expfunc,
            timeout,
            running: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn new(timeout: time::Duration, expfunc: F) -> Self {
        Self::with_capacity(timeout, expfunc, 0)
    }
    pub fn reserve(&self, additional: usize) {
        self.inner.blocking_lock().reserve(additional);
    }
    // if this method returns true, must run expire_loop in a separate thread
    pub fn push(&self, key: K, value: V) 
        where Self: 'static {
        let expvalue = ExpValue {
            value,
            expireat: time::SystemTime::now() + self.timeout,
        };
        self.inner.blocking_lock().push_back(key, expvalue);
        if self.running.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            tokio::task::spawn(self.clone().expire_loop());
        }
    }
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.blocking_lock().get(key).is_some()
    }
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.blocking_lock().get(key).map(|v| v.value.clone())
    }
    pub fn len(&self) -> usize {
        self.inner.blocking_lock().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.blocking_lock().remove(key).map(|v| v.value)
    }
    // task which runs expiration loop, terminates when queue is empty
    async fn expire_loop(self) { 
        let mut inner = self.inner.lock().await;
        while let Some((key, ev)) = inner.pop_front() {
            drop(inner);
            let now = time::SystemTime::now();
            if ev.expireat > now {
                tokio::time::sleep(ev.expireat.duration_since(now).unwrap()).await;
            }
            (self.expfunc)(key, ev.value);
            inner = self.inner.lock().await;
        }
        self.running.store(false, Ordering::Release);
    }
}
