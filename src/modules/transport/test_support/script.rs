use std::{
    collections::VecDeque,
    fmt::Debug,
    sync::{Arc, Mutex},
};

struct State<C, R> {
    expected: VecDeque<(C, R)>,
    calls: Vec<C>,
}

/// An exact ordered script and an inspection handle shared with the adapter.
pub struct Script<C, R>(Arc<Mutex<State<C, R>>>);

impl<C, R> Clone for Script<C, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C: Clone + Debug + PartialEq, R> Script<C, R> {
    pub fn new(expected: impl IntoIterator<Item = (C, R)>) -> Self {
        Self(Arc::new(Mutex::new(State {
            expected: expected.into_iter().collect(),
            calls: Vec::new(),
        })))
    }

    pub fn call(&self, actual: C) -> R {
        let next = {
            let mut state = self.0.lock().unwrap();
            state.calls.push(actual.clone());
            state.expected.pop_front()
        };
        // Assert outside the lock: failed expectations must not poison the log.
        let (expected, reply) = next.unwrap_or_else(|| panic!("unexpected call: {actual:?}"));
        assert_eq!(actual, expected, "script call mismatch");
        reply
    }

    pub fn calls(&self) -> Vec<C> {
        self.0.lock().unwrap().calls.clone()
    }

    pub fn assert_finished(&self) {
        assert!(
            self.0.lock().unwrap().expected.is_empty(),
            "unconsumed script expectations"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_and_recorded_calls_follow_the_exact_script() {
        let script = Script::new([(vec![1, 2], 10), (vec![3], 20)]);
        let adapter = script.clone();
        assert_eq!(adapter.call(vec![1, 2]), 10);
        assert_eq!(adapter.call(vec![3]), 20);
        assert_eq!(script.calls(), [vec![1, 2], vec![3]]);
        script.assert_finished();
    }

    #[test]
    #[should_panic(expected = "script call mismatch")]
    fn wrong_input_or_order_fails_instead_of_returning_success() {
        Script::new([(vec![1, 2], ())]).call(vec![2, 1]);
    }

    #[test]
    #[should_panic(expected = "unexpected call")]
    fn exhausted_scripts_do_not_provide_default_replies() {
        Script::<u8, ()>::new([]).call(1);
    }

    #[test]
    #[should_panic(expected = "unconsumed script expectations")]
    fn omitted_calls_are_detected_at_test_completion() {
        Script::new([(1, ())]).assert_finished();
    }
}
