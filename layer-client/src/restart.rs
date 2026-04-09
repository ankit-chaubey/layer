use std::time::Duration;

pub trait ConnectionRestartPolicy: Send + Sync + 'static {
    fn restart_interval(&self) -> Option<Duration>;
}

pub struct NeverRestart;

impl ConnectionRestartPolicy for NeverRestart {
    fn restart_interval(&self) -> Option<Duration> {
        None
    }
}

pub struct FixedInterval {
    pub interval: Duration,
}

impl ConnectionRestartPolicy for FixedInterval {
    fn restart_interval(&self) -> Option<Duration> {
        Some(self.interval)
    }
}
