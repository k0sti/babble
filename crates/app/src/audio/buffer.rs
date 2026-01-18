use ringbuf::{traits::*, HeapRb};
use std::sync::Arc;
use parking_lot::Mutex;

/// Thread-safe ring buffer for audio samples
pub struct AudioRingBuffer {
    buffer: Arc<Mutex<HeapRb<f32>>>,
}

impl AudioRingBuffer {
    /// Create a new ring buffer with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(HeapRb::new(capacity))),
        }
    }

    /// Write samples to the buffer
    /// Returns the number of samples actually written
    pub fn write(&mut self, samples: &[f32]) -> usize {
        let mut buffer = self.buffer.lock();
        let mut written = 0;

        for &sample in samples {
            if buffer.try_push(sample).is_ok() {
                written += 1;
            } else {
                // Buffer is full, drop old samples
                let _ = buffer.try_pop();
                let _ = buffer.try_push(sample);
                written += 1;
            }
        }

        written
    }

    /// Read up to `count` samples from the buffer
    pub fn read(&mut self, count: usize) -> Vec<f32> {
        let mut buffer = self.buffer.lock();
        let mut samples = Vec::with_capacity(count);

        for _ in 0..count {
            if let Some(sample) = buffer.try_pop() {
                samples.push(sample);
            } else {
                break;
            }
        }

        samples
    }

    /// Get the number of samples available to read
    pub fn len(&self) -> usize {
        let buffer = self.buffer.lock();
        buffer.occupied_len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().is_empty()
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.lock().clear();
    }

    /// Get the capacity of the buffer
    pub fn capacity(&self) -> usize {
        self.buffer.lock().capacity().get()
    }
}

impl Clone for AudioRingBuffer {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read() {
        let mut buffer = AudioRingBuffer::new(1024);
        let data: Vec<f32> = (0..100).map(|i| i as f32).collect();

        let written = buffer.write(&data);
        assert_eq!(written, 100);

        let read_data = buffer.read(100);
        assert_eq!(read_data.len(), 100);
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_overflow() {
        let mut buffer = AudioRingBuffer::new(10);
        let data: Vec<f32> = (0..20).map(|i| i as f32).collect();

        let written = buffer.write(&data);
        assert_eq!(written, 20);

        // Should only be able to read 10 samples (capacity)
        let read_data = buffer.read(20);
        assert_eq!(read_data.len(), 10);
    }
}
