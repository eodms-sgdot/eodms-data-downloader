use std::sync::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

// https://gist.github.com/NoraCodes/e6d40782b05dc8ac40faf3a0405debd3

#[derive(Clone)]
pub struct WorkQueue<T: Send + Clone> {
    inner: Arc<Mutex<VecDeque<T>>>,
}

impl<T: Send + Clone> WorkQueue<T> {
    pub fn new() -> Self { 
        Self { inner: Arc::new(Mutex::new(VecDeque::new())) } 
    }
    pub fn get_work(&self) -> Option<T> {
        let maybe_queue = self.inner.lock();
        if let Ok(mut queue) = maybe_queue {
            queue.pop_front()
        } else {
            panic!("WorkQueue::get_work() tried to lock a poisoned mutex");
        }
    }
    pub fn add_work(&self, work: T) -> usize {
        // As above, try to get a lock on the mutex. 
        if let Ok(mut queue) = self.inner.lock() {
            queue.push_back(work);
            queue.len()
        } else {
            panic!("WorkQueue::add_work() tried to lock a poisoned mutex");
        }
    }
}

pub struct SyncFlagTx {
    inner: Arc<Mutex<bool>>,
}

impl SyncFlagTx {
    pub fn set(&mut self, state: bool) -> Result<(), ()> {
        if let Ok(mut v) = self.inner.lock() {
            *v = state;
            Ok(())
        } else {
            Err(())
        }
    }
}

#[derive(Clone)]
pub struct SyncFlagRx {
    inner: Arc<Mutex<bool>>,
}

impl SyncFlagRx {
    pub fn get(&self) -> Result<bool, ()> {
        if let Ok(v) = self.inner.lock() {
            Ok(*v)
        } else {
            Err(())
        }
    }
}

pub fn new_syncflag(initial_state: bool) -> (SyncFlagTx, SyncFlagRx) {
    let state = Arc::new(Mutex::new(initial_state));
    let tx = SyncFlagTx { inner: state.clone() };
    let rx = SyncFlagRx { inner: state.clone() };

    return (tx, rx);
}

