//! Diagnostics: FPS counter, frame time histogram, system profiling,
//! entity count tracking, and custom diagnostic channels.

use crate::plugin::Plugin;
use crate::App;
use arachne_ecs::{Res, ResMut, Stage};
use crate::time::Time;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Diagnostic resource
// ---------------------------------------------------------------------------

/// Number of frame samples kept for the rolling average / histogram.
const FRAME_HISTORY_SIZE: usize = 120;

/// Diagnostic data updated each frame.
pub struct Diagnostics {
    /// Ring buffer of recent frame times (seconds).
    frame_times: [f32; FRAME_HISTORY_SIZE],
    /// Write cursor into `frame_times`.
    cursor: usize,
    /// Total frames recorded (saturates at usize::MAX).
    total_frames: usize,

    /// Cached FPS (rolling average over last 60 samples).
    fps: f32,
    /// Cached average frame time (seconds).
    avg_frame_time: f32,
    /// Minimum frame time in the history window.
    min_frame_time: f32,
    /// Maximum frame time in the history window.
    max_frame_time: f32,

    /// Current entity count (updated by system each frame).
    entity_count: u32,

    /// Per-system timing data. Keys are system names.
    system_timings: HashMap<String, SystemTiming>,

    /// Custom diagnostic channels. Keys are channel names.
    custom_channels: HashMap<String, DiagnosticChannel>,
}

/// Timing information for a single system.
#[derive(Clone, Debug)]
pub struct SystemTiming {
    /// Most recent execution time in seconds.
    pub last_duration: f32,
    /// Rolling average execution time in seconds.
    pub avg_duration: f32,
    /// Maximum recorded execution time in seconds.
    pub max_duration: f32,
    /// Number of times this system has been timed.
    pub sample_count: u64,
    /// Sum of all durations for average computation.
    total_duration: f64,
}

impl SystemTiming {
    fn new() -> Self {
        Self {
            last_duration: 0.0,
            avg_duration: 0.0,
            max_duration: 0.0,
            sample_count: 0,
            total_duration: 0.0,
        }
    }

    fn record(&mut self, duration_secs: f32) {
        self.last_duration = duration_secs;
        self.total_duration += duration_secs as f64;
        self.sample_count += 1;
        self.avg_duration = (self.total_duration / self.sample_count as f64) as f32;
        if duration_secs > self.max_duration {
            self.max_duration = duration_secs;
        }
    }
}

/// A custom diagnostic channel that tracks a named floating-point value
/// over time (e.g., memory usage, draw call count, etc.).
#[derive(Clone, Debug)]
pub struct DiagnosticChannel {
    /// Ring buffer of recent values.
    values: Vec<f32>,
    /// Write cursor.
    cursor: usize,
    /// Total samples recorded.
    sample_count: usize,
    /// Capacity of the ring buffer.
    capacity: usize,
    /// Cached statistics.
    pub current: f32,
    pub average: f32,
    pub min: f32,
    pub max: f32,
}

impl DiagnosticChannel {
    fn new(capacity: usize) -> Self {
        Self {
            values: vec![0.0; capacity],
            cursor: 0,
            sample_count: 0,
            capacity,
            current: 0.0,
            average: 0.0,
            min: 0.0,
            max: 0.0,
        }
    }

    fn record(&mut self, value: f32) {
        self.values[self.cursor] = value;
        self.cursor = (self.cursor + 1) % self.capacity;
        self.sample_count += 1;
        self.current = value;
        self.recompute();
    }

    fn recompute(&mut self) {
        let count = self.sample_count.min(self.capacity);
        if count == 0 {
            return;
        }

        let mut sum = 0.0f32;
        let mut min = f32::MAX;
        let mut max = f32::MIN;

        for i in 0..count {
            let idx = if self.sample_count >= self.capacity {
                (self.cursor + i) % self.capacity
            } else {
                i
            };
            let v = self.values[idx];
            sum += v;
            if v < min { min = v; }
            if v > max { max = v; }
        }

        self.average = sum / count as f32;
        self.min = min;
        self.max = max;
    }

    /// Get values in chronological order (oldest first).
    pub fn values_ordered(&self) -> Vec<f32> {
        let count = self.sample_count.min(self.capacity);
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            let idx = if self.sample_count >= self.capacity {
                (self.cursor + i) % self.capacity
            } else {
                i
            };
            result.push(self.values[idx]);
        }
        result
    }
}

impl Diagnostics {
    pub fn new() -> Self {
        Self {
            frame_times: [0.0; FRAME_HISTORY_SIZE],
            cursor: 0,
            total_frames: 0,
            fps: 0.0,
            avg_frame_time: 0.0,
            min_frame_time: 0.0,
            max_frame_time: 0.0,
            entity_count: 0,
            system_timings: HashMap::new(),
            custom_channels: HashMap::new(),
        }
    }

    /// Record a frame time and recompute cached stats.
    pub fn record_frame(&mut self, dt: f32) {
        self.frame_times[self.cursor] = dt;
        self.cursor = (self.cursor + 1) % FRAME_HISTORY_SIZE;
        self.total_frames = self.total_frames.saturating_add(1);

        self.recompute();
    }

    fn recompute(&mut self) {
        let count = self.total_frames.min(FRAME_HISTORY_SIZE);
        if count == 0 {
            return;
        }

        let mut sum = 0.0f32;
        let mut min = f32::MAX;
        let mut max = f32::MIN;

        let fps_window = count.min(60);

        for i in 0..count {
            let idx = (self.cursor + FRAME_HISTORY_SIZE - 1 - i) % FRAME_HISTORY_SIZE;
            let t = self.frame_times[idx];
            sum += t;
            if t < min {
                min = t;
            }
            if t > max {
                max = t;
            }
        }

        self.avg_frame_time = sum / count as f32;
        self.min_frame_time = min;
        self.max_frame_time = max;

        let mut fps_sum = 0.0f32;
        for i in 0..fps_window {
            let idx = (self.cursor + FRAME_HISTORY_SIZE - 1 - i) % FRAME_HISTORY_SIZE;
            fps_sum += self.frame_times[idx];
        }
        if fps_sum > 0.0 {
            self.fps = fps_window as f32 / fps_sum;
        }
    }

    // -- Accessors --------------------------------------------------------

    /// Frames per second (rolling average over last 60 frames).
    #[inline]
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Average frame time in seconds (over the full history window).
    #[inline]
    pub fn avg_frame_time(&self) -> f32 {
        self.avg_frame_time
    }

    /// Minimum frame time in the history window.
    #[inline]
    pub fn min_frame_time(&self) -> f32 {
        self.min_frame_time
    }

    /// Maximum frame time in the history window.
    #[inline]
    pub fn max_frame_time(&self) -> f32 {
        self.max_frame_time
    }

    /// Total number of frames recorded.
    #[inline]
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// Read-only access to the frame time histogram (last 120 frame times).
    pub fn frame_times(&self) -> &[f32; FRAME_HISTORY_SIZE] {
        &self.frame_times
    }

    /// Returns frame times in chronological order (oldest first).
    pub fn frame_times_ordered(&self) -> Vec<f32> {
        let count = self.total_frames.min(FRAME_HISTORY_SIZE);
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            let idx = if self.total_frames >= FRAME_HISTORY_SIZE {
                (self.cursor + i) % FRAME_HISTORY_SIZE
            } else {
                i
            };
            result.push(self.frame_times[idx]);
        }
        result
    }

    // -- Entity count -----------------------------------------------------

    /// Current number of live entities.
    #[inline]
    pub fn entity_count(&self) -> u32 {
        self.entity_count
    }

    /// Update the entity count (called by the diagnostic system).
    pub fn set_entity_count(&mut self, count: u32) {
        self.entity_count = count;
    }

    // -- System timing ----------------------------------------------------

    /// Record a system's execution duration.
    pub fn record_system_timing(&mut self, system_name: &str, duration_secs: f32) {
        self.system_timings
            .entry(system_name.to_string())
            .or_insert_with(SystemTiming::new)
            .record(duration_secs);
    }

    /// Get timing data for a specific system.
    pub fn system_timing(&self, system_name: &str) -> Option<&SystemTiming> {
        self.system_timings.get(system_name)
    }

    /// Get all system timings.
    pub fn all_system_timings(&self) -> &HashMap<String, SystemTiming> {
        &self.system_timings
    }

    /// Total time spent in systems this frame (sum of last_duration).
    pub fn total_system_time(&self) -> f32 {
        self.system_timings
            .values()
            .map(|t| t.last_duration)
            .sum()
    }

    /// Returns the N slowest systems sorted by average duration.
    pub fn slowest_systems(&self, n: usize) -> Vec<(&str, &SystemTiming)> {
        let mut systems: Vec<(&str, &SystemTiming)> = self
            .system_timings
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect();
        systems.sort_by(|a, b| {
            b.1.avg_duration
                .partial_cmp(&a.1.avg_duration)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        systems.truncate(n);
        systems
    }

    // -- Custom diagnostic channels ---------------------------------------

    /// Register a custom diagnostic channel with the given history capacity.
    pub fn register_channel(&mut self, name: &str, capacity: usize) {
        self.custom_channels
            .entry(name.to_string())
            .or_insert_with(|| DiagnosticChannel::new(capacity));
    }

    /// Record a value to a custom channel.
    pub fn record_channel(&mut self, name: &str, value: f32) {
        if let Some(channel) = self.custom_channels.get_mut(name) {
            channel.record(value);
        }
    }

    /// Get a custom diagnostic channel.
    pub fn channel(&self, name: &str) -> Option<&DiagnosticChannel> {
        self.custom_channels.get(name)
    }

    // -- Summary generation -----------------------------------------------

    /// Generate a text summary of current diagnostics.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("FPS: {:.1}\n", self.fps));
        s.push_str(&format!(
            "Frame time: {:.2}ms (avg) / {:.2}ms (min) / {:.2}ms (max)\n",
            self.avg_frame_time * 1000.0,
            self.min_frame_time * 1000.0,
            self.max_frame_time * 1000.0
        ));
        s.push_str(&format!("Entities: {}\n", self.entity_count));
        s.push_str(&format!("Total frames: {}\n", self.total_frames));

        if !self.system_timings.is_empty() {
            s.push_str("System timings:\n");
            let slowest = self.slowest_systems(10);
            for (name, timing) in slowest {
                s.push_str(&format!(
                    "  {}: {:.3}ms avg, {:.3}ms max\n",
                    name,
                    timing.avg_duration * 1000.0,
                    timing.max_duration * 1000.0
                ));
            }
        }

        s
    }

    /// Check if the frame budget (16.6ms for 60fps) is being met.
    pub fn is_frame_budget_ok(&self) -> bool {
        self.avg_frame_time < (1.0 / 60.0) * 1.1 // 10% tolerance
    }

    /// Returns the percentage of the frame budget used (0-100+).
    pub fn frame_budget_usage(&self) -> f32 {
        (self.avg_frame_time / (1.0 / 60.0)) * 100.0
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Diagnostic system
// ---------------------------------------------------------------------------

/// System that records the current frame's delta time into diagnostics.
fn diagnostic_update_system(time: Res<Time>, mut diag: ResMut<Diagnostics>) {
    diag.record_frame(time.delta_seconds());
}

// ---------------------------------------------------------------------------
// DiagnosticPlugin
// ---------------------------------------------------------------------------

/// Plugin that registers the [`Diagnostics`] resource and the system that
/// records frame times.
pub struct DiagnosticPlugin;

impl Plugin for DiagnosticPlugin {
    fn build(&self, app: &mut App) {
        if !app.world.has_resource::<Diagnostics>() {
            app.world.insert_resource(Diagnostics::new());
        }
        app.schedule
            .add_system(Stage::PreUpdate, diagnostic_update_system);
    }

    fn name(&self) -> &str {
        "DiagnosticPlugin"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_reports_60_at_60fps() {
        let mut diag = Diagnostics::new();
        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            diag.record_frame(dt);
        }
        let fps = diag.fps();
        assert!(
            (fps - 60.0).abs() < 1.0,
            "Expected ~60 FPS, got {}",
            fps
        );
    }

    #[test]
    fn frame_times_ordered() {
        let mut diag = Diagnostics::new();
        for i in 0..10 {
            diag.record_frame(i as f32 * 0.001);
        }
        let ordered = diag.frame_times_ordered();
        assert_eq!(ordered.len(), 10);
        for i in 0..10 {
            assert!((ordered[i] - i as f32 * 0.001).abs() < 1e-6);
        }
    }

    #[test]
    fn min_max_frame_time() {
        let mut diag = Diagnostics::new();
        diag.record_frame(0.01);
        diag.record_frame(0.02);
        diag.record_frame(0.005);
        assert!((diag.min_frame_time() - 0.005).abs() < 1e-6);
        assert!((diag.max_frame_time() - 0.02).abs() < 1e-6);
    }

    // -- Entity count tests -----------------------------------------------

    #[test]
    fn entity_count_tracking() {
        let mut diag = Diagnostics::new();
        assert_eq!(diag.entity_count(), 0);
        diag.set_entity_count(42);
        assert_eq!(diag.entity_count(), 42);
    }

    // -- System timing tests ----------------------------------------------

    #[test]
    fn system_timing_recording() {
        let mut diag = Diagnostics::new();
        diag.record_system_timing("physics_step", 0.002);
        diag.record_system_timing("physics_step", 0.003);
        diag.record_system_timing("render", 0.008);

        let timing = diag.system_timing("physics_step").unwrap();
        assert_eq!(timing.sample_count, 2);
        assert!((timing.last_duration - 0.003).abs() < 1e-6);
        assert!((timing.max_duration - 0.003).abs() < 1e-6);
        assert!((timing.avg_duration - 0.0025).abs() < 1e-6);
    }

    #[test]
    fn slowest_systems() {
        let mut diag = Diagnostics::new();
        diag.record_system_timing("fast", 0.001);
        diag.record_system_timing("slow", 0.010);
        diag.record_system_timing("medium", 0.005);

        let slowest = diag.slowest_systems(2);
        assert_eq!(slowest.len(), 2);
        assert_eq!(slowest[0].0, "slow");
        assert_eq!(slowest[1].0, "medium");
    }

    #[test]
    fn total_system_time() {
        let mut diag = Diagnostics::new();
        diag.record_system_timing("a", 0.002);
        diag.record_system_timing("b", 0.003);

        let total = diag.total_system_time();
        assert!((total - 0.005).abs() < 1e-6);
    }

    // -- Custom channel tests ---------------------------------------------

    #[test]
    fn custom_channel_recording() {
        let mut diag = Diagnostics::new();
        diag.register_channel("draw_calls", 60);

        diag.record_channel("draw_calls", 10.0);
        diag.record_channel("draw_calls", 20.0);
        diag.record_channel("draw_calls", 15.0);

        let channel = diag.channel("draw_calls").unwrap();
        assert!((channel.current - 15.0).abs() < 1e-6);
        assert!((channel.average - 15.0).abs() < 1e-6);
        assert!((channel.min - 10.0).abs() < 1e-6);
        assert!((channel.max - 20.0).abs() < 1e-6);
    }

    #[test]
    fn custom_channel_values_ordered() {
        let mut diag = Diagnostics::new();
        diag.register_channel("memory", 10);

        for i in 0..5 {
            diag.record_channel("memory", i as f32 * 100.0);
        }

        let channel = diag.channel("memory").unwrap();
        let vals = channel.values_ordered();
        assert_eq!(vals.len(), 5);
        assert_eq!(vals[0], 0.0);
        assert_eq!(vals[4], 400.0);
    }

    #[test]
    fn unregistered_channel_ignored() {
        let mut diag = Diagnostics::new();
        diag.record_channel("nonexistent", 42.0);
        assert!(diag.channel("nonexistent").is_none());
    }

    // -- Summary / budget tests -------------------------------------------

    #[test]
    fn summary_contains_fps() {
        let mut diag = Diagnostics::new();
        diag.record_frame(1.0 / 60.0);
        let summary = diag.summary();
        assert!(summary.contains("FPS:"));
        assert!(summary.contains("Entities:"));
    }

    #[test]
    fn frame_budget_ok_at_60fps() {
        let mut diag = Diagnostics::new();
        for _ in 0..60 {
            diag.record_frame(1.0 / 60.0);
        }
        assert!(diag.is_frame_budget_ok());
    }

    #[test]
    fn frame_budget_exceeded_at_30fps() {
        let mut diag = Diagnostics::new();
        for _ in 0..60 {
            diag.record_frame(1.0 / 30.0);
        }
        assert!(!diag.is_frame_budget_ok());
    }

    #[test]
    fn frame_budget_usage() {
        let mut diag = Diagnostics::new();
        for _ in 0..60 {
            diag.record_frame(1.0 / 60.0);
        }
        let usage = diag.frame_budget_usage();
        assert!(
            (usage - 100.0).abs() < 5.0,
            "Expected ~100%, got {}%",
            usage
        );
    }

    #[test]
    fn system_timing_missing_returns_none() {
        let diag = Diagnostics::new();
        assert!(diag.system_timing("nonexistent").is_none());
    }

    #[test]
    fn all_system_timings() {
        let mut diag = Diagnostics::new();
        diag.record_system_timing("a", 0.001);
        diag.record_system_timing("b", 0.002);
        assert_eq!(diag.all_system_timings().len(), 2);
    }
}
