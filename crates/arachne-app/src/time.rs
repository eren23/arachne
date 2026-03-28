//! Time management: frame delta, elapsed time, fixed timestep, stopwatch, timer.

// ---------------------------------------------------------------------------
// Time resource
// ---------------------------------------------------------------------------

/// Global time resource updated each frame by the runner.
pub struct Time {
    /// Seconds elapsed since the last frame.
    delta: f32,
    /// Total seconds elapsed since the app started.
    elapsed: f32,
    /// Number of frames that have been rendered.
    frame_count: u64,

    // Fixed timestep accumulator for physics.
    fixed_delta: f32,
    accumulator: f32,
}

impl Time {
    /// Creates a new `Time` resource with a default fixed timestep of 1/60s.
    pub fn new() -> Self {
        Self {
            delta: 0.0,
            elapsed: 0.0,
            frame_count: 0,
            fixed_delta: 1.0 / 60.0,
            accumulator: 0.0,
        }
    }

    /// Advances time by the given raw delta (called by the runner each frame).
    pub fn update(&mut self, raw_delta: f32) {
        // Clamp to avoid spiral of death (e.g. after a long pause).
        let dt = raw_delta.min(0.25);
        self.delta = dt;
        self.elapsed += dt;
        self.frame_count += 1;
        self.accumulator += dt;
    }

    /// Delta time for the current frame (seconds).
    #[inline]
    pub fn delta_seconds(&self) -> f32 {
        self.delta
    }

    /// Total elapsed time since the app started (seconds).
    #[inline]
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed
    }

    /// Number of frames that have been rendered.
    #[inline]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// The fixed timestep interval (seconds). Default: 1/60.
    #[inline]
    pub fn fixed_delta(&self) -> f32 {
        self.fixed_delta
    }

    /// Set the fixed timestep interval.
    pub fn set_fixed_delta(&mut self, dt: f32) {
        assert!(dt > 0.0, "fixed_delta must be > 0");
        self.fixed_delta = dt;
    }

    /// Returns `true` if there is enough accumulated time for a fixed step,
    /// and consumes one step's worth of time from the accumulator.
    pub fn consume_fixed_step(&mut self) -> bool {
        if self.accumulator >= self.fixed_delta {
            self.accumulator -= self.fixed_delta;
            true
        } else {
            false
        }
    }

    /// Interpolation alpha for rendering between fixed steps (0..1).
    #[inline]
    pub fn interpolation_alpha(&self) -> f32 {
        if self.fixed_delta > 0.0 {
            self.accumulator / self.fixed_delta
        } else {
            1.0
        }
    }

    /// Reset time to initial state.
    pub fn reset(&mut self) {
        self.delta = 0.0;
        self.elapsed = 0.0;
        self.frame_count = 0;
        self.accumulator = 0.0;
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Stopwatch
// ---------------------------------------------------------------------------

/// A stopwatch that can be started, paused, and reset.
pub struct Stopwatch {
    elapsed: f32,
    paused: bool,
}

impl Stopwatch {
    pub fn new() -> Self {
        Self {
            elapsed: 0.0,
            paused: true,
        }
    }

    /// Creates a stopwatch that is already running.
    pub fn started() -> Self {
        Self {
            elapsed: 0.0,
            paused: false,
        }
    }

    /// Advance the stopwatch by `dt` seconds (only if not paused).
    pub fn tick(&mut self, dt: f32) {
        if !self.paused {
            self.elapsed += dt;
        }
    }

    /// Total elapsed time in seconds.
    #[inline]
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// Start or resume the stopwatch.
    pub fn start(&mut self) {
        self.paused = false;
    }

    /// Pause the stopwatch.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Reset elapsed time to zero. Does not change paused state.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }

    /// Is the stopwatch currently paused?
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.paused
    }
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Timer
// ---------------------------------------------------------------------------

/// A countdown timer with optional repeating behaviour.
pub struct Timer {
    duration: f32,
    elapsed: f32,
    repeating: bool,
    finished: bool,
    /// Number of times the timer has completed (useful for repeating timers).
    times_finished: u32,
}

impl Timer {
    /// Create a new timer with the given duration in seconds.
    pub fn new(duration: f32, repeating: bool) -> Self {
        assert!(duration > 0.0, "Timer duration must be > 0");
        Self {
            duration,
            elapsed: 0.0,
            repeating,
            finished: false,
            times_finished: 0,
        }
    }

    /// Advance the timer by `dt` seconds. Returns `true` if the timer
    /// finished (or wrapped around) during this tick.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.finished && !self.repeating {
            return false;
        }

        self.times_finished = 0;
        self.elapsed += dt;

        if self.elapsed >= self.duration {
            if self.repeating {
                while self.elapsed >= self.duration {
                    self.elapsed -= self.duration;
                    self.times_finished += 1;
                }
                self.finished = false;
                true
            } else {
                self.elapsed = self.duration;
                self.finished = true;
                self.times_finished = 1;
                true
            }
        } else {
            self.finished = false;
            false
        }
    }

    /// Has the timer finished?
    #[inline]
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// How many times the timer completed during the last `tick()` call.
    #[inline]
    pub fn times_finished(&self) -> u32 {
        self.times_finished
    }

    /// The timer's duration in seconds.
    #[inline]
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Elapsed time in seconds.
    #[inline]
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// Fraction of progress toward completion (0.0 .. 1.0).
    #[inline]
    pub fn fraction(&self) -> f32 {
        (self.elapsed / self.duration).min(1.0)
    }

    /// Reset the timer.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.finished = false;
        self.times_finished = 0;
    }

    /// Is this a repeating timer?
    #[inline]
    pub fn repeating(&self) -> bool {
        self.repeating
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_basic_update() {
        let mut time = Time::new();
        time.update(1.0 / 60.0);
        assert!((time.delta_seconds() - 1.0 / 60.0).abs() < 1e-6);
        assert_eq!(time.frame_count(), 1);
    }

    #[test]
    fn time_elapsed_accumulates() {
        let mut time = Time::new();
        let dt = 1.0 / 60.0;
        for _ in 0..5 {
            time.update(dt);
        }
        let expected = dt * 5.0;
        assert!((time.elapsed_seconds() - expected).abs() < 1e-5);
        assert_eq!(time.frame_count(), 5);
    }

    #[test]
    fn time_clamps_large_delta() {
        let mut time = Time::new();
        time.update(10.0); // huge spike
        assert!((time.delta_seconds() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn time_fixed_step_accumulation() {
        let mut time = Time::new();
        time.set_fixed_delta(1.0 / 60.0);
        time.update(1.0 / 30.0); // two fixed steps worth
        let mut steps = 0;
        while time.consume_fixed_step() {
            steps += 1;
        }
        assert_eq!(steps, 2);
    }

    #[test]
    fn time_interpolation_alpha() {
        let mut time = Time::new();
        time.set_fixed_delta(1.0 / 60.0);
        time.update(1.0 / 60.0 * 1.5); // 1.5 steps
        assert!(time.consume_fixed_step());
        let alpha = time.interpolation_alpha();
        assert!((alpha - 0.5).abs() < 0.01);
    }

    #[test]
    fn stopwatch_basic() {
        let mut sw = Stopwatch::started();
        sw.tick(0.5);
        sw.tick(0.5);
        assert!((sw.elapsed() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn stopwatch_pause_resume() {
        let mut sw = Stopwatch::started();
        sw.tick(1.0);
        sw.pause();
        sw.tick(1.0);
        assert!((sw.elapsed() - 1.0).abs() < 1e-6);
        sw.start();
        sw.tick(1.0);
        assert!((sw.elapsed() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn stopwatch_reset() {
        let mut sw = Stopwatch::started();
        sw.tick(5.0);
        sw.reset();
        assert!((sw.elapsed()).abs() < 1e-6);
        assert!(!sw.is_paused());
    }

    #[test]
    fn timer_oneshot() {
        let mut timer = Timer::new(1.0, false);
        assert!(!timer.tick(0.5));
        assert!(!timer.finished());
        assert!(timer.tick(0.6));
        assert!(timer.finished());
        assert_eq!(timer.times_finished(), 1);
        // Further ticks do nothing.
        assert!(!timer.tick(1.0));
    }

    #[test]
    fn timer_repeating() {
        let mut timer = Timer::new(1.0, true);
        assert!(timer.tick(2.5)); // wraps twice, 0.5 remaining
        assert_eq!(timer.times_finished(), 2);
        assert!(!timer.finished());
        assert!((timer.elapsed() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn timer_fraction() {
        let mut timer = Timer::new(2.0, false);
        timer.tick(1.0);
        assert!((timer.fraction() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn timer_reset() {
        let mut timer = Timer::new(1.0, false);
        timer.tick(1.5);
        assert!(timer.finished());
        timer.reset();
        assert!(!timer.finished());
        assert!((timer.elapsed()).abs() < 1e-6);
    }
}
