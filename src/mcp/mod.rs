//! MCP (Model Context Protocol) server for Claude/Cursor integration.
//!
//! Exposes tools: analyze_test_quality, suggest_improvements, get_mutation_score.

use crate::analyzer::AnalysisEngine;
use crate::suggestions::AiSuggestionGenerator;
use crate::mutation;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// MCP JSON-RPC request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

/// MCP JSON-RPC response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
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
                        description: "Analyze a test file and return quality score and issues".to_string(),
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
                        description: "Run fast mutation testing on the test file's source and return kill rate".to_string(),
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
                let (name, args_obj) = req.params
                    .as_ref()
                    .and_then(|p| p.get("params").or(Some(p)))
                    .and_then(|p| {
                        let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let args = p.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
                        let obj = args.as_object().cloned().unwrap_or_default();
                        Some((name, obj))
                    })
                    .unwrap_or(("", serde_json::Map::new()));
                let file = args_obj.get("file").and_then(|v| v.as_str()).unwrap_or("").to_string();
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

        let response = JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result,
            error: None,
        };
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
