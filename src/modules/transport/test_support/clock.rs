use std::{
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use crate::modules::transport::ports::io::Clock;

struct State {
    now: Instant,
    timer: Option<(Instant, Waker)>,
}

/// Clone the control handle before moving a clock into Transport dependencies.
/// Only explicit advance changes time; the supplied origin is otherwise opaque.
#[derive(Clone)]
pub struct ManualClock(Arc<Mutex<State>>);

#[derive(Debug, PartialEq, Eq)]
pub struct ClockOverflow;

impl ManualClock {
    pub fn new(origin: Instant) -> Self {
        Self(Arc::new(Mutex::new(State {
            now: origin,
            timer: None,
        })))
    }

    pub fn advance(&self, delta: Duration) -> Result<(), ClockOverflow> {
        let wake = {
            let mut state = self.0.lock().unwrap();
            state.now = state.now.checked_add(delta).ok_or(ClockOverflow)?;
            if state
                .timer
                .as_ref()
                .is_some_and(|(deadline, _)| *deadline <= state.now)
            {
                state.timer.take().map(|(_, waker)| waker)
            } else {
                None
            }
        };
        if let Some(waker) = wake {
            waker.wake();
        }
        Ok(())
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.0.lock().unwrap().now
    }

    fn poll_deadline(&mut self, cx: &mut Context<'_>, deadline: Instant) -> Poll<()> {
        let mut state = self.0.lock().unwrap();
        if state.now >= deadline {
            state.timer = None;
            Poll::Ready(())
        } else {
            state.timer = Some((deadline, cx.waker().clone()));
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::wake::WakeCounter;
    use super::*;

    #[test]
    fn time_advances_only_explicitly_and_wakes_at_the_deadline_once() {
        let origin = Instant::now();
        let mut clock = ManualClock::new(origin);
        let control = clock.clone();
        let counter = Arc::new(WakeCounter::default());
        let waker = counter.waker();
        let mut cx = Context::from_waker(&waker);
        let deadline = origin + Duration::from_secs(10);
        assert!(clock.poll_deadline(&mut cx, deadline).is_pending());
        assert_eq!(clock.now(), origin);
        control.advance(Duration::from_secs(9)).unwrap();
        assert_eq!(counter.count(), 0);
        control.advance(Duration::from_secs(1)).unwrap();
        assert_eq!(counter.count(), 1);
        assert!(clock.poll_deadline(&mut cx, deadline).is_ready());
        control.advance(Duration::from_secs(1)).unwrap();
        assert_eq!(counter.count(), 1);
    }

    #[test]
    fn replacing_a_timer_or_waker_discards_the_previous_registration() {
        let origin = Instant::now();
        let mut clock = ManualClock::new(origin);
        let first = Arc::new(WakeCounter::default());
        let second = Arc::new(WakeCounter::default());
        assert!(
            clock
                .poll_deadline(
                    &mut Context::from_waker(&first.waker()),
                    origin + Duration::from_secs(1)
                )
                .is_pending()
        );
        assert!(
            clock
                .poll_deadline(
                    &mut Context::from_waker(&second.waker()),
                    origin + Duration::from_secs(2)
                )
                .is_pending()
        );
        clock.advance(Duration::from_secs(1)).unwrap();
        assert_eq!(first.count() + second.count(), 0);
        clock.advance(Duration::from_secs(1)).unwrap();
        assert_eq!(first.count(), 0);
        assert_eq!(second.count(), 1);
    }

    #[test]
    fn ready_deadlines_clear_old_timers_and_overflow_leaves_time_unchanged() {
        let origin = Instant::now();
        let mut clock = ManualClock::new(origin);
        let counter = Arc::new(WakeCounter::default());
        let waker = counter.waker();
        let mut cx = Context::from_waker(&waker);
        assert!(
            clock
                .poll_deadline(&mut cx, origin + Duration::from_secs(1))
                .is_pending()
        );
        assert!(clock.poll_deadline(&mut cx, origin).is_ready());
        assert_eq!(clock.advance(Duration::MAX), Err(ClockOverflow));
        assert_eq!(clock.now(), origin);
        clock.advance(Duration::from_secs(1)).unwrap();
        assert_eq!(counter.count(), 0);
    }
}
