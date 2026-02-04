//! Claude API integration for AI-powered test improvements
//!
//! Requires the `ai` feature to be enabled:
//! ```toml
//! rigor = { version = "0.1", features = ["ai"] }
//! ```

use crate::AnalysisResult;

/// Claude API client for generating test improvements
#[allow(dead_code)]
pub struct ClaudeClient {
    api_key: String,
    model: String,
    base_url: String,
}

/// Result from Claude API
#[derive(Debug)]
pub struct ClaudeResponse {
    pub improved_code: String,
    pub tokens_used: Option<u32>,
}

/// Error from Claude API
#[derive(Debug)]
pub enum ClaudeError {
    NoApiKey,
    RequestFailed(String),
    InvalidResponse(String),
    RateLimited,
    ApiError(String),
}

impl std::fmt::Display for ClaudeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaudeError::NoApiKey => write!(f, "ANTHROPIC_API_KEY environment variable not set"),
            ClaudeError::RequestFailed(e) => write!(f, "Request failed: {}", e),
            ClaudeError::InvalidResponse(e) => write!(f, "Invalid response: {}", e),
            ClaudeError::RateLimited => write!(f, "Rate limited - try again later"),
            ClaudeError::ApiError(e) => write!(f, "API error: {}", e),
        }
    }
}

impl std::error::Error for ClaudeError {}

impl ClaudeClient {
    /// Create a new Claude client using ANTHROPIC_API_KEY from environment
    pub fn from_env() -> Result<Self, ClaudeError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ClaudeError::NoApiKey)?;
        
        Ok(Self {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            base_url: "https://api.anthropic.com/v1/messages".to_string(),
        })
    }

    /// Create a client with a specific API key
    pub fn with_key(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            base_url: "https://api.anthropic.com/v1/messages".to_string(),
        }
    }

    /// Set the model to use
    pub fn model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    /// Improve a test file using Claude
    #[cfg(feature = "ai")]
    pub fn improve_tests(&self, result: &AnalysisResult) -> Result<ClaudeResponse, ClaudeError> {
        use super::AiSuggestionGenerator;
        
        let generator = AiSuggestionGenerator::new();
        let prompt = generator.generate_prompt(result);
        
        self.send_request(&prompt)
    }

    /// Send a prompt to Claude and get the response
    #[cfg(feature = "ai")]
    pub fn send_request(&self, prompt: &str) -> Result<ClaudeResponse, ClaudeError> {
        use serde_json::json;
        
        let client = reqwest::blocking::Client::new();
        
        let body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let response = client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .map_err(|e| ClaudeError::RequestFailed(e.to_string()))?;

        let status = response.status();
        
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ClaudeError::RateLimited);
        }
        
        if !status.is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(ClaudeError::ApiError(format!("{}: {}", status, error_text)));
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| ClaudeError::InvalidResponse(e.to_string()))?;

        // Extract the text from Claude's response
        let content = json["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["text"].as_str())
            .ok_or_else(|| ClaudeError::InvalidResponse("No content in response".to_string()))?;

        // Extract code block from response
        let code = super::extract_code_block(content)
            .unwrap_or_else(|| content.to_string());

        let tokens = json["usage"]["output_tokens"].as_u64().map(|t| t as u32);

        Ok(ClaudeResponse {
            improved_code: code,
            tokens_used: tokens,
        })
    }

    /// Stub implementation when ai feature is disabled
    #[cfg(not(feature = "ai"))]
    pub fn improve_tests(&self, _result: &AnalysisResult) -> Result<ClaudeResponse, ClaudeError> {
        Err(ClaudeError::RequestFailed(
            "AI feature not enabled. Rebuild with: cargo build --features ai".to_string()
        ))
    }

    #[cfg(not(feature = "ai"))]
    pub fn send_request(&self, _prompt: &str) -> Result<ClaudeResponse, ClaudeError> {
        Err(ClaudeError::RequestFailed(
            "AI feature not enabled. Rebuild with: cargo build --features ai".to_string()
        ))
    }
}

/// Check if the AI feature is available
pub fn is_ai_available() -> bool {
    cfg!(feature = "ai")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_api_key() {
        // Temporarily unset the key
        std::env::remove_var("ANTHROPIC_API_KEY");
        let result = ClaudeClient::from_env();
        assert!(matches!(result, Err(ClaudeError::NoApiKey)));
    }
}
