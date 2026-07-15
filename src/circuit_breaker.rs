use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use time::OffsetDateTime;

#[derive(Debug)]
pub enum CircuitBreakerState {
    Open,
    HalfOpen,
    Closed,
}

#[derive(Debug)]
pub struct CircuitBreaker {
    state: Mutex<CircuitBreakerState>,
    failure_count: AtomicU8,
    failure_limit: u8,
    opened_at: AtomicU64,
    recovery_timeout_ms: u64,
    half_open_calls: AtomicU8,
    half_open_max_calls: u8,
}

impl CircuitBreaker {
    pub fn new(failure_limit: u8, recovery_timeout_ms: u64, half_open_max_calls: u8) -> Self {
        Self {
            state: Mutex::new(CircuitBreakerState::Closed),
            failure_count: AtomicU8::new(0),
            failure_limit,
            opened_at: AtomicU64::new(0),
            recovery_timeout_ms,
            half_open_calls: AtomicU8::new(0),
            half_open_max_calls,
        }
    }

    pub fn allow_request(&self) -> bool {
        let mut current_state = self.state.lock().unwrap();

        match *current_state {
            CircuitBreakerState::Open => {
                let current_timestamp = (OffsetDateTime::now_utc().unix_timestamp() * 1000) as u64;
                let circuit_opened_at = self.opened_at.load(Ordering::SeqCst);

                if current_timestamp - circuit_opened_at > self.recovery_timeout_ms {
                    self.half_open_calls.store(0, Ordering::SeqCst); //reset the counter
                    self.half_open_calls.fetch_add(1, Ordering::SeqCst);
                    *current_state = CircuitBreakerState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => {
                let current_half_open_calls = self.half_open_calls.fetch_add(1, Ordering::SeqCst);
                if current_half_open_calls >= self.half_open_max_calls {
                    self.half_open_calls.fetch_sub(1, Ordering::SeqCst);
                    false
                } else {
                    true
                }
            }
            CircuitBreakerState::Closed => true,
        }
    }

    pub fn record_success(&self) {
        let mut current_state = self.state.lock().unwrap();
        match *current_state {
            CircuitBreakerState::HalfOpen => {
                *current_state = CircuitBreakerState::Closed;
                self.opened_at.store(0, Ordering::SeqCst);
                self.half_open_calls.store(0, Ordering::SeqCst);
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitBreakerState::Closed => {
                self.failure_count.store(0, Ordering::SeqCst);
            }
            _ => (),
        }
    }

    pub fn record_failure(&self) {
        let mut current_state = self.state.lock().unwrap();

        match *current_state {
            CircuitBreakerState::Closed => {
                let new_count = self.failure_count.fetch_add(1, Ordering::SeqCst);
                if new_count >= self.failure_limit {
                    *current_state = CircuitBreakerState::Open;
                    let current_timestamp = OffsetDateTime::now_utc().unix_timestamp() * 1000;
                    self.opened_at
                        .store(current_timestamp as u64, Ordering::SeqCst);
                    self.half_open_calls.store(0, Ordering::SeqCst);
                }
            }
            CircuitBreakerState::HalfOpen => {
                *current_state = CircuitBreakerState::Open;
                let current_timestamp = OffsetDateTime::now_utc().unix_timestamp() * 1000;
                self.opened_at
                    .store(current_timestamp as u64, Ordering::SeqCst);
                self.half_open_calls.store(0, Ordering::SeqCst);
                self.failure_count.store(0, Ordering::SeqCst);
            }
            _ => (),
        }
    }
}
