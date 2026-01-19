//! UI Components for Babble
//!
//! This module contains all the reusable UI components.

mod message_list;
mod audio_player;
mod waveform;
mod input_bar;
mod debug_panel;
mod status_bar;

pub use message_list::MessageList;
pub use audio_player::AudioPlayer;
pub use waveform::Waveform;
pub use input_bar::InputBar;
pub use debug_panel::DebugPanel;
pub use status_bar::{StatusBar, CompactStatusBar, ProcessorStatus, StatusIndicator};
