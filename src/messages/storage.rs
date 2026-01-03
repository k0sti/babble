use super::types::Message;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct MessageStorage {
    messages: Arc<RwLock<Vec<Message>>>,
}

impl MessageStorage {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add(&self, message: Message) {
        self.messages.write().push(message);
    }

    pub fn get_all(&self) -> Vec<Message> {
        self.messages.read().clone()
    }

    pub fn clear(&self) {
        self.messages.write().clear();
    }

    pub fn len(&self) -> usize {
        self.messages.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.read().is_empty()
    }
}

impl Default for MessageStorage {
    fn default() -> Self {
        Self::new()
    }
}
