//! Exponential backoff strategy for retry policies.

use std::time::Duration;

#[derive(Clone)]
pub struct ExponentialBackoff {
    initial: Duration,
    max: Duration,
    factor: f64,
    jitter: f64,
}

impl ExponentialBackoff {
    pub fn new(initial: Duration, max: Duration, factor: f64) -> Self {
        Self {
            initial,
            max,
            factor,
            jitter: 0.1,
        }
    }

    pub fn with_jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter.clamp(0.0, 1.0);
        self
    }

    pub fn delay_for(&self, attempt: u32) -> Duration {
        let base =
            self.initial.as_millis() as f64 * self.factor.powi(attempt.saturating_sub(1) as i32);
        let clamped = base.min(self.max.as_millis() as f64);

        let jittered = if self.jitter > 0.0 {
            let jitter_range = clamped * self.jitter;
            let jitter_offset = rand::random::<f64>() * jitter_range * 2.0 - jitter_range;
            (clamped + jitter_offset).max(0.0)
        } else {
            clamped
        };

        Duration::from_millis(jittered as u64)
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(30),
            factor: 2.0,
            jitter: 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(10), 2.0)
                .with_jitter(0.0);

        assert_eq!(backoff.delay_for(1), Duration::from_millis(100));
        assert_eq!(backoff.delay_for(2), Duration::from_millis(200));
        assert_eq!(backoff.delay_for(3), Duration::from_millis(400));
        assert_eq!(backoff.delay_for(4), Duration::from_millis(800));
    }

    #[test]
    fn test_exponential_backoff_max() {
        let backoff =
            ExponentialBackoff::new(Duration::from_millis(100), Duration::from_millis(500), 2.0)
                .with_jitter(0.0);

        assert_eq!(backoff.delay_for(10), Duration::from_millis(500));
    }
}
