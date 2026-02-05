//! MCP (Model Context Protocol) server for Claude/Cursor integration.
//!
//! Exposes tools: analyze_test_quality, suggest_improvements, get_mutation_score.

use crate::analyzer::AnalysisEngine;
use crate::mutation;
use crate::suggestions::AiSuggestionGenerator;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// MCP JSON-RPC request
#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: Option<String>,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// MCP JSON-RPC response
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// Tool definition for MCP tools/list
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolDef {
    name: String,
    description: String,
    input_schema: InputSchema,
}

#[derive(Debug, Serialize)]
struct InputSchema {
    #[serde(rename = "type")]
    typ: &'static str,
    properties: serde_json::Value,
    required: Vec<&'static str>,
}

/// Handle a single JSON-RPC request and return a response.
/// Extracted from `run_mcp_server` for testability.
pub fn handle_request(req: &JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone();
    let result = match req.method.as_str() {
        "initialize" => Some(serde_json::json!({
            "protocolVersion": "0.1.0",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "rigor", "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => {
            let tools = vec![
                ToolDef {
                    name: "analyze_test_quality".to_string(),
                    description: "Analyze a test file and return quality score and issues"
                        .to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "file": { "type": "string", "description": "Path to test file (.test.ts, .spec.ts)" }
                        }),
                        required: vec!["file"],
                    },
                },
                ToolDef {
                    name: "suggest_improvements".to_string(),
                    description: "Generate an AI prompt to improve the given test file".to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "file": { "type": "string", "description": "Path to test file" }
                        }),
                        required: vec!["file"],
                    },
                },
                ToolDef {
                    name: "get_mutation_score".to_string(),
                    description:
                        "Run fast mutation testing on the test file's source and return kill rate"
                            .to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "file": { "type": "string", "description": "Path to test file" },
                            "count": { "type": "number", "description": "Max mutants to run (default 10)" }
                        }),
                        required: vec!["file"],
                    },
                },
            ];
            Some(serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let (name, args_obj) = req
                .params
                .as_ref()
                .and_then(|p| p.get("params").or(Some(p)))
                .map(|p| {
                    let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let args = p
                        .get("arguments")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let obj = args.as_object().cloned().unwrap_or_default();
                    (name, obj)
                })
                .unwrap_or(("", serde_json::Map::new()));
            let file = args_obj
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let count = args_obj.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            let result = match name {
                "analyze_test_quality" => run_analyze(&file),
                "suggest_improvements" => run_suggest(&file),
                "get_mutation_score" => run_mutation_score(&file, count),
                _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
            };

            match result {
                Ok(val) => Some(serde_json::json!({
                    "content": [{ "type": "text", "text": serde_json::to_string(&val).unwrap_or_else(|_| "{}".to_string()) }]
                })),
                Err(e) => Some(serde_json::json!({
                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                    "isError": true
                })),
            }
        }
        _ => None,
    };

    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result,
        error: None,
    }
}

/// Run the MCP server loop (stdin / stdout).
pub fn run_mcp_server() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let response = handle_request(&req);
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn run_analyze(file: &str) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(file);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }
    let engine = AnalysisEngine::new();
    let result = engine.analyze(path, None)?;
    Ok(serde_json::json!({
        "filePath": result.file_path,
        "score": result.score,
        "breakdown": result.breakdown,
        "issues": result.issues,
        "stats": result.stats,
    }))
}

fn run_suggest(file: &str) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(file);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }
    let engine = AnalysisEngine::new();
    let result = engine.analyze(path, None)?;
    let gen = AiSuggestionGenerator::new();
    let prompt = gen.generate_prompt(&result);
    Ok(serde_json::json!({ "prompt": prompt }))
}

fn run_mutation_score(file: &str, count: usize) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(file);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }
    let engine = AnalysisEngine::new();
    let result = engine.analyze(path, None)?;
    let source_path = match &result.source_file {
        Some(p) if p.exists() => p.clone(),
        _ => anyhow::bail!("No source file found for {}", file),
    };
    let content = std::fs::read_to_string(&source_path)?;
    let test_cmd = std::env::var("RIGOR_TEST_CMD").unwrap_or_else(|_| "npm test".to_string());
    let mutation_result = mutation::run_mutation_test(&source_path, &content, &test_cmd, count)?;
    Ok(serde_json::json!({
        "total": mutation_result.total,
        "killed": mutation_result.killed,
        "survived": mutation_result.survived,
        "scorePercent": if mutation_result.total > 0 { (mutation_result.killed as f32 / mutation_result.total as f32 * 100.0) as u32 } else { 0 }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: &str, params: Option<serde_json::Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(serde_json::json!(1)),
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn test_initialize_returns_protocol_version_and_server_info() {
        let req = make_request("initialize", None);
        let resp = handle_request(&req);

        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(serde_json::json!(1)));
        assert!(resp.error.is_none());

        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "0.1.0");
        assert_eq!(result["serverInfo"]["name"], "rigor");
        assert!(result["serverInfo"]["version"].is_string());
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_tools_list_returns_three_tools() {
        let req = make_request("tools/list", None);
        let resp = handle_request(&req);

        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);

        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"analyze_test_quality"));
        assert!(names.contains(&"suggest_improvements"));
        assert!(names.contains(&"get_mutation_score"));

        // Each tool must have inputSchema with type, properties, required
        for tool in tools {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object");
            assert!(schema["properties"].is_object());
            assert!(schema["required"].is_array());

            let required = schema["required"].as_array().unwrap();
            assert!(
                required.iter().any(|r| r == "file"),
                "every tool should require 'file' param"
            );
        }
    }

    #[test]
    fn test_tools_call_analyze_nonexistent_file_returns_error() {
        let req = make_request(
            "tools/call",
            Some(serde_json::json!({
                "name": "analyze_test_quality",
                "arguments": { "file": "/nonexistent/path/test.ts" }
            })),
        );
        let resp = handle_request(&req);

        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("Error"),
            "expected error message, got: {}",
            text
        );
    }

    #[test]
    fn test_tools_call_unknown_tool_returns_error() {
        let req = make_request(
            "tools/call",
            Some(serde_json::json!({
                "name": "nonexistent_tool",
                "arguments": { "file": "test.ts" }
            })),
        );
        let resp = handle_request(&req);

        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Unknown tool"));
    }

    #[test]
    fn test_unknown_method_returns_null_result() {
        let req = make_request("nonexistent/method", None);
        let resp = handle_request(&req);

        assert!(resp.result.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_tools_call_analyze_real_file() {
        // Use a fixture file that exists in the repo
        let req = make_request(
            "tools/call",
            Some(serde_json::json!({
                "name": "analyze_test_quality",
                "arguments": { "file": "test-repos/fake-project/tests/auth.test.ts" }
            })),
        );
        let resp = handle_request(&req);

        let result = resp.result.unwrap();
        assert!(
            result.get("isError").is_none(),
            "expected success, got: {:?}",
            result
        );

        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed.get("filePath").is_some());
        assert!(parsed.get("score").is_some());
        assert!(parsed.get("breakdown").is_some());
        assert!(parsed.get("issues").is_some());
    }

    #[test]
    fn test_jsonrpc_request_parsing() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"initialize","params":null}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(serde_json::json!(42)));
    }

    #[test]
    fn test_jsonrpc_request_with_string_id() {
        let json = r#"{"jsonrpc":"2.0","id":"abc-123","method":"tools/list"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(serde_json::json!("abc-123")));
        let resp = handle_request(&req);
        assert_eq!(resp.id, Some(serde_json::json!("abc-123")));
    }

    #[test]
    fn test_jsonrpc_request_without_id() {
        // Notifications have no id
        let json = r#"{"jsonrpc":"2.0","method":"initialize"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert!(req.id.is_none());
        let resp = handle_request(&req);
        assert!(resp.id.is_none());
    }

    #[test]
    fn test_tools_call_with_nested_params() {
        // Some MCP clients wrap params inside a "params" key
        let req = make_request(
            "tools/call",
            Some(serde_json::json!({
                "params": {
                    "name": "analyze_test_quality",
                    "arguments": { "file": "/nonexistent/file.test.ts" }
                }
            })),
        );
        let resp = handle_request(&req);
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }
}
