//! Deterministic durable-cut fixture; never linked into a production provider.
use super::Error;
use std::collections::VecDeque;

/// Scripted atomic replacement boundary.
#[derive(Clone, Copy)]
pub(super) enum Cut {
    Before,
    After,
    None,
}
/// Durable bytes survive a simulated restart; replies do not.
pub(super) struct Register {
    pub(super) durable: Vec<u8>,
    cuts: VecDeque<Cut>,
}
impl Register {
    // Construct an independent store with an explicit finite failure script.
    pub(super) fn new(durable: Vec<u8>, cuts: impl IntoIterator<Item = Cut>) -> Self {
        Self {
            durable,
            cuts: cuts.into_iter().collect(),
        }
    }
    // Replace atomically, distinguishing refusal, lost acknowledgement and script exhaustion.
    pub(super) fn replace(&mut self, bytes: &[u8]) -> Result<(), Error> {
        match self.cuts.pop_front().ok_or(Error::Unavailable)? {
            Cut::Before => Err(Error::Storage),
            Cut::After => {
                self.durable = bytes.to_vec();
                Err(Error::Unknown)
            }
            Cut::None => {
                self.durable = bytes.to_vec();
                Ok(())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Restart observes committed bytes even when the original writer never received success.
    #[test]
    fn atomic_cuts_and_exhaustion() {
        let mut r = Register::new(vec![1], [Cut::Before, Cut::After, Cut::None]);
        assert_eq!(r.replace(&[2]), Err(Error::Storage));
        assert_eq!(r.durable, [1]);
        assert_eq!(r.replace(&[3]), Err(Error::Unknown));
        let reopened = Register::new(r.durable.clone(), []);
        assert_eq!(reopened.durable, [3]);
        assert_eq!(r.replace(&[4]), Ok(()));
        assert_eq!(r.replace(&[5]), Err(Error::Unavailable));
        assert_eq!(r.durable, [4]);
    }
}

/// Monotonic fixture clock with explicit overflow refusal.
pub(super) struct Clock(pub(super) u64);
impl Clock {
    // Advance milliseconds without sleeps or wraparound.
    pub(super) fn advance(&mut self, milliseconds: u64) -> Result<(), Error> {
        self.0 = self.0.checked_add(milliseconds).ok_or(Error::Limit)?;
        Ok(())
    }
}
/// Contract-neutral dependency script; every call consumes exactly one explicit response.
pub(super) struct Script<T> {
    replies: VecDeque<Result<T, Error>>,
    pub(super) calls: usize,
}
impl<T> Script<T> {
    // Retain literal expectations without deriving them from production calculations.
    pub(super) fn new(replies: impl IntoIterator<Item = Result<T, Error>>) -> Self {
        Self {
            replies: replies.into_iter().collect(),
            calls: 0,
        }
    }
    // Fail closed when no dependency response was explicitly configured.
    pub(super) fn next(&mut self) -> Result<T, Error> {
        self.calls += 1;
        self.replies.pop_front().unwrap_or(Err(Error::Unavailable))
    }
}
#[cfg(test)]
mod script_tests {
    use super::*;
    // Literal handler/peer responses preserve success, refusal and ambiguous outcomes independently.
    #[test]
    fn scripted_ports_and_clock() {
        let mut handler = Script::new([Ok((vec![0, 255], 7u64)), Err(Error::Unknown)]);
        assert_eq!(handler.next(), Ok((vec![0, 255], 7)));
        assert_eq!(handler.next(), Err(Error::Unknown));
        assert_eq!(handler.next(), Err(Error::Unavailable));
        assert_eq!(handler.calls, 3);
        let mut body = Script::new([Ok(vec![1, 2]), Err(Error::Unavailable)]);
        assert_eq!(body.next(), Ok(vec![1, 2]));
        assert_eq!(body.next(), Err(Error::Unavailable));
        let mut clock = Clock(0);
        clock.advance(1201).unwrap();
        assert_eq!(clock.0, 1201);
        clock.advance(u64::MAX).unwrap_err();
        assert_eq!(clock.0, 1201);
    }
}

pub(super) mod family;
