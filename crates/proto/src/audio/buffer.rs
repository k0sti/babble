//! Thread-safe ring buffer for audio samples
//!
//! Implements a circular buffer that automatically drops oldest samples
//! when capacity is exceeded, suitable for real-time audio streaming.

use parking_lot::Mutex;
use ringbuf::{traits::*, HeapRb};
use std::sync::Arc;

/// Thread-safe ring buffer for audio samples
///
/// Uses an internal Arc<Mutex<HeapRb>> to allow safe sharing across threads.
/// When the buffer is full, writing new samples automatically drops the oldest ones.
#[derive(Clone)]
pub struct AudioRingBuffer {
    buffer: Arc<Mutex<HeapRb<f32>>>,
}

impl AudioRingBuffer {
    /// Create a new ring buffer with the specified capacity
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of f32 samples the buffer can hold
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(HeapRb::new(capacity))),
        }
    }

    /// Write samples to the buffer
    ///
    /// If the buffer is full, oldest samples are automatically dropped
    /// to make room for new ones.
    ///
    /// # Arguments
    /// * `samples` - Slice of f32 samples to write
    ///
    /// # Returns
    /// The number of samples written (always equals samples.len())
    pub fn write(&self, samples: &[f32]) -> usize {
        let mut buffer = self.buffer.lock();

        for &sample in samples {
            if buffer.try_push(sample).is_err() {
                // Buffer is full, drop oldest sample to make room
                let _ = buffer.try_pop();
                let _ = buffer.try_push(sample);
            }
        }

        samples.len()
    }

    /// Read up to `count` samples from the buffer
    ///
    /// # Arguments
    /// * `count` - Maximum number of samples to read
    ///
    /// # Returns
    /// A Vec containing the read samples (may be fewer than `count` if buffer has less)
    pub fn read(&self, count: usize) -> Vec<f32> {
        let mut buffer = self.buffer.lock();
        let mut samples = Vec::with_capacity(count.min(buffer.occupied_len()));

        for _ in 0..count {
            if let Some(sample) = buffer.try_pop() {
                samples.push(sample);
            } else {
                break;
            }
        }

        samples
    }

    /// Clear all samples from the buffer
    pub fn clear(&self) {
        self.buffer.lock().clear();
    }

    /// Get the number of samples currently in the buffer
    pub fn len(&self) -> usize {
        self.buffer.lock().occupied_len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().is_empty()
    }

    /// Get the capacity of the buffer
    pub fn capacity(&self) -> usize {
        self.buffer.lock().capacity().get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buffer = AudioRingBuffer::new(1024);
        assert_eq!(buffer.capacity(), 1024);
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_write_read() {
        let buffer = AudioRingBuffer::new(1024);
        let data: Vec<f32> = (0..100).map(|i| i as f32).collect();

        let written = buffer.write(&data);
        assert_eq!(written, 100);
        assert_eq!(buffer.len(), 100);

        let read_data = buffer.read(100);
        assert_eq!(read_data.len(), 100);
        assert_eq!(read_data, data);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_overflow_drops_oldest() {
        let buffer = AudioRingBuffer::new(10);
        let data: Vec<f32> = (0..20).map(|i| i as f32).collect();

        let written = buffer.write(&data);
        assert_eq!(written, 20);

        // Buffer should contain only the last 10 samples
        let read_data = buffer.read(20);
        assert_eq!(read_data.len(), 10);
        // Should have samples 10-19 (oldest dropped)
        let expected: Vec<f32> = (10..20).map(|i| i as f32).collect();
        assert_eq!(read_data, expected);
    }

    #[test]
    fn test_clear() {
        let buffer = AudioRingBuffer::new(100);
        buffer.write(&[1.0, 2.0, 3.0]);
        assert_eq!(buffer.len(), 3);

        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_clone_shares_buffer() {
        let buffer1 = AudioRingBuffer::new(100);
        let buffer2 = buffer1.clone();

        buffer1.write(&[1.0, 2.0, 3.0]);
        assert_eq!(buffer2.len(), 3);

        let data = buffer2.read(3);
        assert_eq!(data, vec![1.0, 2.0, 3.0]);
        assert!(buffer1.is_empty());
    }

    #[test]
    fn test_partial_read() {
        let buffer = AudioRingBuffer::new(100);
        buffer.write(&[1.0, 2.0, 3.0, 4.0, 5.0]);

        let data = buffer.read(3);
        assert_eq!(data, vec![1.0, 2.0, 3.0]);
        assert_eq!(buffer.len(), 2);

        let remaining = buffer.read(10);
        assert_eq!(remaining, vec![4.0, 5.0]);
    }
}
