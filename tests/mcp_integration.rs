//! Integration tests for the MCP server public API.
//! Exercises handle_request from outside the crate (initialize, tools/list, tools/call error paths).

use rigor::mcp::{handle_request, JsonRpcRequest};
use serde_json::json;

fn make_request(method: &str, params: Option<serde_json::Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: Some("2.0".to_string()),
        id: Some(json!(1)),
        method: method.to_string(),
        params,
    }
}

#[test]
fn mcp_initialize_returns_protocol_and_server_info() {
    let req = make_request("initialize", None);
    let resp = handle_request(&req);

    assert_eq!(resp.jsonrpc, "2.0");
    assert!(resp.error.is_none());
    let result = resp.result.expect("expected result");
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "rigor");
    assert!(result["serverInfo"]["version"].as_str().is_some());
}

#[test]
fn mcp_tools_list_returns_all_tools() {
    let req = make_request("tools/list", None);
    let resp = handle_request(&req);

    assert!(resp.error.is_none());
    let result = resp.result.expect("expected result");
    let tools = result["tools"].as_array().expect("tools array");
    assert!(
        tools.len() >= 9,
        "expected at least 9 tools, got {}",
        tools.len()
    );
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"analyze_test_quality"));
    assert!(names.contains(&"suggest_improvements"));
}

#[test]
fn mcp_tools_call_nonexistent_file_returns_error_content() {
    let req = make_request(
        "tools/call",
        Some(json!({
            "name": "analyze_test_quality",
            "arguments": { "file": "/nonexistent/path/does-not-exist.test.ts" }
        })),
    );
    let resp = handle_request(&req);

    assert!(resp.error.is_none());
    let result = resp.result.expect("expected result");
    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.to_lowercase().contains("error") || text.contains("No such file"));
}
