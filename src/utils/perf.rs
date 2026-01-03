//! Performance monitoring utilities
//!
//! Provides tools for measuring and tracking performance metrics.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Tracks timing metrics over a sliding window
#[derive(Debug)]
pub struct TimingTracker {
    samples: VecDeque<Duration>,
    max_samples: usize,
}

impl TimingTracker {
    /// Create a new timing tracker with the specified window size
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Record a new timing sample
    pub fn record(&mut self, duration: Duration) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(duration);
    }

    /// Get the average duration
    pub fn average(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.samples.iter().sum();
        total / self.samples.len() as u32
    }

    /// Get the minimum duration
    pub fn min(&self) -> Duration {
        self.samples.iter().min().copied().unwrap_or(Duration::ZERO)
    }

    /// Get the maximum duration
    pub fn max(&self) -> Duration {
        self.samples.iter().max().copied().unwrap_or(Duration::ZERO)
    }

    /// Get the 95th percentile duration
    pub fn percentile_95(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let mut sorted: Vec<_> = self.samples.iter().copied().collect();
        sorted.sort();
        let idx = (sorted.len() as f32 * 0.95) as usize;
        sorted.get(idx.min(sorted.len() - 1)).copied().unwrap_or(Duration::ZERO)
    }

    /// Get the number of samples
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// Clear all samples
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

/// A simple stopwatch for measuring elapsed time
#[derive(Debug)]
pub struct Stopwatch {
    start: Instant,
    splits: Vec<(String, Duration)>,
}

impl Stopwatch {
    /// Start a new stopwatch
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
            splits: Vec::new(),
        }
    }

    /// Record a split time with a label
    pub fn split(&mut self, label: impl Into<String>) {
        self.splits.push((label.into(), self.start.elapsed()));
    }

    /// Get the elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Get all recorded splits
    pub fn splits(&self) -> &[(String, Duration)] {
        &self.splits
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

/// Performance metrics for the application
#[derive(Debug, Default, Clone)]
pub struct PerformanceMetrics {
    /// STT processing time (ms)
    pub stt_latency_ms: Option<u64>,

    /// LLM time to first token (ms)
    pub llm_ttft_ms: Option<u64>,

    /// LLM total generation time (ms)
    pub llm_total_ms: Option<u64>,

    /// TTS synthesis time (ms)
    pub tts_latency_ms: Option<u64>,

    /// Audio playback buffer size
    pub audio_buffer_size: usize,

    /// Current frame rate
    pub fps: f32,

    /// Memory usage (bytes)
    pub memory_usage: Option<usize>,
}

impl PerformanceMetrics {
    /// Calculate the total voice-to-voice latency
    pub fn total_latency_ms(&self) -> Option<u64> {
        match (self.stt_latency_ms, self.llm_ttft_ms, self.tts_latency_ms) {
            (Some(stt), Some(ttft), Some(tts)) => Some(stt + ttft + tts),
            _ => None,
        }
    }

    /// Check if performance targets are met
    pub fn meets_targets(&self) -> bool {
        // Target: < 1 second total latency
        if let Some(total) = self.total_latency_ms() {
            total < 1000
        } else {
            true // No data yet, assume good
        }
    }

    /// Generate a performance summary string
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(stt) = self.stt_latency_ms {
            parts.push(format!("STT: {}ms", stt));
        }
        if let Some(ttft) = self.llm_ttft_ms {
            parts.push(format!("TTFT: {}ms", ttft));
        }
        if let Some(tts) = self.tts_latency_ms {
            parts.push(format!("TTS: {}ms", tts));
        }
        if let Some(total) = self.total_latency_ms() {
            parts.push(format!("Total: {}ms", total));
        }

        parts.push(format!("FPS: {:.0}", self.fps));

        parts.join(" | ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_tracker() {
        let mut tracker = TimingTracker::new(10);

        for i in 1..=5 {
            tracker.record(Duration::from_millis(i * 10));
        }

        assert_eq!(tracker.count(), 5);
        assert_eq!(tracker.min(), Duration::from_millis(10));
        assert_eq!(tracker.max(), Duration::from_millis(50));
        assert_eq!(tracker.average(), Duration::from_millis(30));
    }

    #[test]
    fn test_timing_tracker_window() {
        let mut tracker = TimingTracker::new(3);

        for i in 1..=5 {
            tracker.record(Duration::from_millis(i * 10));
        }

        // Should only have last 3 samples
        assert_eq!(tracker.count(), 3);
        assert_eq!(tracker.min(), Duration::from_millis(30));
    }

    #[test]
    fn test_stopwatch() {
        let mut sw = Stopwatch::start();
        std::thread::sleep(Duration::from_millis(10));
        sw.split("first");
        std::thread::sleep(Duration::from_millis(10));
        sw.split("second");

        assert!(sw.elapsed() >= Duration::from_millis(20));
        assert_eq!(sw.splits().len(), 2);
    }

    #[test]
    fn test_performance_metrics() {
        let metrics = PerformanceMetrics {
            stt_latency_ms: Some(200),
            llm_ttft_ms: Some(150),
            tts_latency_ms: Some(100),
            llm_total_ms: Some(500),
            audio_buffer_size: 1024,
            fps: 60.0,
            memory_usage: None,
        };

        assert_eq!(metrics.total_latency_ms(), Some(450));
        assert!(metrics.meets_targets());
        assert!(!metrics.summary().is_empty());
    }
}
