use std::time::Duration;
use wasm_timer::Instant;
#[derive(Debug)]
pub struct Statistics {
    statistics_tick: Duration,
    begin_timestamp: Instant,
    last_timestamp: Instant,
    last_statistics_timestamp: Instant,

    times: u32,
    frame_duration: Duration,

    frame_secends: f32,
    frame_count: u32,
    fps: f32,
    target_frame_secends: Option<Duration>,
    changed: bool,
}

impl Statistics {
    pub fn new(tick: Duration, target_frame_secends: Option<f32>) -> Self {
        let now = Instant::now();
        Self {
            statistics_tick: tick,
            begin_timestamp: now,
            last_timestamp: now,
            last_statistics_timestamp: now,

            times: 0,
            frame_duration: Duration::from_secs(0),

            frame_count: 0,
            frame_secends: 0f32,
            fps: 0f32,

            target_frame_secends: target_frame_secends.map(|v| Duration::from_secs_f32(v)),
            changed: false,
        }
    }

    pub fn set_frame_lock(&mut self, target_frame_seconds: Option<f32>) {
        self.target_frame_secends = target_frame_seconds.map(|v| Duration::from_secs_f32(v));
    }

    pub fn new_frame(&mut self) -> bool {
        let now = Instant::now();
        let delta = now - self.last_timestamp;

        if let Some(target) = self.target_frame_secends {
            if target > delta {
                return false;
            }
        }
        self.times += 1;
        self.changed = false;

        let pass = now - self.last_statistics_timestamp;
        if pass >= self.statistics_tick || self.fps == 0f32 {
            self.frame_secends = (pass.as_micros() as f32 / 1000_000f32) / self.times as f32;
            self.fps = 1.0f32 / self.frame_secends;
            self.frame_count = self.times;

            self.times = 0;
            self.last_statistics_timestamp = now;
            self.changed = true;
        }
        self.last_timestamp = now;
        self.frame_duration = delta;
        return true;
    }

    pub fn next_frame(&self) -> (Instant, bool) {
        let now = Instant::now();
        match self.target_frame_secends {
            Some(target) => {
                let d = now - self.last_timestamp;
                if target > d {
                    return (now + (target - d), false);
                }
                (now, true)
            }
            None => (now, true),
        }
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }

    pub fn frame_secends(&self) -> f32 {
        self.frame_secends
    }

    pub fn elapsed(&self) -> Duration {
        self.last_timestamp - self.begin_timestamp
    }

    pub fn changed(&self) -> bool {
        self.changed
    }
}
