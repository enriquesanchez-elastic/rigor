//! Suggestions module for AI-powered improvements

pub mod ai;
pub mod applier;
pub mod claude;

pub use ai::AiSuggestionGenerator;
pub use applier::{extract_code_block, offer_apply};
pub use claude::{ClaudeClient, ClaudeError, ClaudeResponse, is_ai_available};
