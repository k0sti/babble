//! End-to-end integration module
//!
//! This module provides the orchestration layer that connects all components
//! of the Babble voice assistant: Voice -> STT -> LLM -> TTS -> Playback

mod orchestrator;
mod config;

pub use orchestrator::{
    Orchestrator,
    OrchestratorHandle,
    OrchestratorCommand,
    OrchestratorEvent,
};
pub use config::IntegrationConfig;
