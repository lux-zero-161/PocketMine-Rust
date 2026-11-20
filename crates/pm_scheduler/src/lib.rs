//! Tick-based task scheduler mirroring `pocketmine\scheduler\TaskScheduler`.
//!
//! All scheduling is measured in server ticks (20 per second). Tasks never run
//! concurrently with world/entity ticking, which keeps the single-threaded
//! world model of PocketMine-MP intact while the Rust core stays fast.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub type TaskId = u64;

/// A unit of work owned by a plugin or the core.
pub trait Task: Send {
    fn on_run(&mut self, current_tick: u64);
    fn on_cancel(&mut self) {}
}

impl<F> Task for F
where
    F: FnMut(u64) + Send,
{
    fn on_run(&mut self, current_tick: u64) {
        self(current_tick)
    }
}

pub struct ScheduledTask {
    pub id: TaskId,
    pub owner: Option<String>,
    pub delay: u64,
    pub period: Option<u64>,
    pub next_run: u64,
    pub cancelled: bool,
    task: Box<dyn Task>,
}

impl ScheduledTask {
    pub fn is_repeating(&self) -> bool {
        self.period.is_some()
    }
}

pub struct TaskScheduler {
    next_id: AtomicU64,
    tasks: HashMap<TaskId, ScheduledTask>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            tasks: HashMap::new(),
        }
    }

    fn insert(
        &mut self,
        owner: Option<String>,
        delay: u64,
        period: Option<u64>,
        task: Box<dyn Task>,
        current_tick: u64,
    ) -> TaskId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        // PMMP treats a delay of 0 as "next tick".
        let delay = delay.max(1);
        self.tasks.insert(
            id,
            ScheduledTask {
                id,
                owner,
                delay,
                period,
                next_run: current_tick + delay,
                cancelled: false,
                task,
            },
        );
        id
    }

    /// Runs a task once after `delay` ticks.
    pub fn schedule_delayed(
        &mut self,
        owner: Option<String>,
        delay: u64,
        task: Box<dyn Task>,
        current_tick: u64,
    ) -> TaskId {
        self.insert(owner, delay, None, task, current_tick)
    }

    /// Runs a task immediately (next tick).
    pub fn schedule(
        &mut self,
        owner: Option<String>,
        task: Box<dyn Task>,
        current_tick: u64,
    ) -> TaskId {
        self.insert(owner, 1, None, task, current_tick)
    }

    /// Runs a task every `period` ticks after an initial `delay`.
    pub fn schedule_repeating(
        &mut self,
        owner: Option<String>,
        delay: u64,
        period: u64,
        task: Box<dyn Task>,
        current_tick: u64,
    ) -> TaskId {
        self.insert(owner, delay, Some(period.max(1)), task, current_tick)
    }

    pub fn cancel(&mut self, id: TaskId) -> bool {
        if let Some(task) = self.tasks.get_mut(&id) {
            task.cancelled = true;
            task.task.on_cancel();
            self.tasks.remove(&id);
            true
        } else {
            false
        }
    }

    /// Cancels every task belonging to `owner` (used when a plugin disables).
    pub fn cancel_all(&mut self, owner: &str) -> usize {
        let ids: Vec<TaskId> = self
            .tasks
            .values()
            .filter(|t| t.owner.as_deref() == Some(owner))
            .map(|t| t.id)
            .collect();
        let count = ids.len();
        for id in ids {
            self.cancel(id);
        }
        count
    }

    pub fn pending(&self) -> usize {
        self.tasks.len()
    }

    /// Advances the scheduler and executes every due task exactly once per tick.
    pub fn tick(&mut self, current_tick: u64) {
        let mut due: Vec<TaskId> = self
            .tasks
            .values()
            .filter(|t| !t.cancelled && t.next_run <= current_tick)
            .map(|t| t.id)
            .collect();
        due.sort_unstable();

        for id in due {
            let Some(task) = self.tasks.get_mut(&id) else {
                continue;
            };
            if task.cancelled {
                continue;
            }
            task.task.on_run(current_tick);

            match task.period {
                Some(period) => {
                    // Guard against runaway catch-up if the server stalled.
                    task.next_run = current_tick + period;
                }
                None => {
                    self.tasks.remove(&id);
                }
            }
        }
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn runs_delayed_and_repeating_tasks() {
        let mut scheduler = TaskScheduler::new();
        let hits = Arc::new(Mutex::new(0u32));
        let h1 = hits.clone();
        scheduler.schedule(
            Some("core".into()),
            Box::new(move |_| *h1.lock().unwrap() += 1),
            0,
        );
        let h2 = hits.clone();
        scheduler.schedule_repeating(
            Some("core".into()),
            1,
            2,
            Box::new(move |_| *h2.lock().unwrap() += 10),
            0,
        );

        scheduler.tick(1);
        assert_eq!(*hits.lock().unwrap(), 11);
        scheduler.tick(2);
        assert_eq!(*hits.lock().unwrap(), 11);
        scheduler.tick(3);
        assert_eq!(*hits.lock().unwrap(), 21);

        assert_eq!(scheduler.cancel_all("core"), 1);
        assert_eq!(scheduler.pending(), 0);
    }
}
