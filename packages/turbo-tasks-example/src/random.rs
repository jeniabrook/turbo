use std::{
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};

use crate::math::I32ValueRef;
use rand::Rng;
use turbo_tasks::Task;

#[turbo_tasks::function]
pub async fn random(id: RandomIdRef) -> I32ValueRef {
    let id = id.await;
    let mut rng = rand::thread_rng();
    let invalidator = Task::get_invalidator();
    let dur = id.duration;
    if id.counter.fetch_sub(1, Ordering::SeqCst) > 1 {
        async_std::task::spawn(async move {
            async_std::task::sleep(dur).await;
            println!("invalidate random number...");
            invalidator.invalidate();
        });
    }
    I32ValueRef::new(rng.gen_range(1..=6))
}

#[turbo_tasks::value]
pub struct RandomId {
    duration: Duration,
    counter: AtomicI32,
}

#[turbo_tasks::value_impl]
impl RandomId {
    #[turbo_tasks::constructor(compare: reuse)]
    pub fn new(duration: Duration, times: i32) -> Self {
        Self {
            duration,
            counter: AtomicI32::new(times),
        }
    }

    fn reuse(&self, duration: &Duration, _times: &i32) -> bool {
        self.duration == *duration
    }
}
