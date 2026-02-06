//! MCP (Model Context Protocol) server for Claude/Cursor integration.
//!
//! Exposes tools: analyze_test_quality, suggest_improvements, get_mutation_score,
//! analyze_with_source, get_improvement_plan, explain_rule, iterate_improvement,
//! get_test_template, compare_tests.

use crate::analyzer::AnalysisEngine;
use crate::mutation;
use crate::parser::{ExportKind, SourceFileParser, TypeScriptParser};
use crate::suggestions::AiSuggestionGenerator;
use crate::{Issue, Rule};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Mutex;

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

/// Session state for iterate_improvement tracking
#[derive(Clone)]
struct ImprovementSession {
    score: u8,
    issue_count: usize,
}

fn improvement_sessions() -> &'static Mutex<HashMap<String, ImprovementSession>> {
    use std::sync::OnceLock;
    static CELL: OnceLock<Mutex<HashMap<String, ImprovementSession>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Handle a single JSON-RPC request and return a response.
/// Extracted from `run_mcp_server` for testability.
pub fn handle_request(req: &JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone();
    let result = match req.method.as_str() {
        "initialize" => Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": true } },
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
                ToolDef {
                    name: "analyze_with_source".to_string(),
                    description: "Analyze a test file together with its source file; returns analysis plus optional source content for context".to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "testFile": { "type": "string", "description": "Path to test file" },
                            "sourceFile": { "type": "string", "description": "Optional path to source file to include in response" }
                        }),
                        required: vec!["testFile"],
                    },
                },
                ToolDef {
                    name: "get_improvement_plan".to_string(),
                    description: "Get a prioritized action plan for improving a test file (issues ordered by severity and category)".to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "file": { "type": "string", "description": "Path to test file" }
                        }),
                        required: vec!["file"],
                    },
                },
                ToolDef {
                    name: "explain_rule".to_string(),
                    description: "Explain a Rigor rule with examples (e.g. weak-assertion, missing-error-test)".to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "ruleId": { "type": "string", "description": "Rule id in kebab-case (e.g. weak-assertion)" }
                        }),
                        required: vec!["ruleId"],
                    },
                },
                ToolDef {
                    name: "iterate_improvement".to_string(),
                    description: "Run analysis and track improvement vs previous call for the same session (score delta, issues resolved)".to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "file": { "type": "string", "description": "Path to test file" },
                            "sessionId": { "type": "string", "description": "Optional session id to track across calls (defaults to file path)" }
                        }),
                        required: vec!["file"],
                    },
                },
                ToolDef {
                    name: "get_test_template".to_string(),
                    description: "Generate a test template for a source file (suggested describe and test cases from exports)".to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "sourceFile": { "type": "string", "description": "Path to source file" }
                        }),
                        required: vec!["sourceFile"],
                    },
                },
                ToolDef {
                    name: "compare_tests".to_string(),
                    description: "Compare two test files (scores, issue counts by rule, summary)".to_string(),
                    input_schema: InputSchema {
                        typ: "object",
                        properties: serde_json::json!({
                            "fileA": { "type": "string", "description": "Path to first test file" },
                            "fileB": { "type": "string", "description": "Path to second test file" }
                        }),
                        required: vec!["fileA", "fileB"],
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
            let test_file = args_obj
                .get("testFile")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let source_file = args_obj
                .get("sourceFile")
                .and_then(|v| v.as_str())
                .map(String::from);
            let rule_id = args_obj
                .get("ruleId")
                .and_then(|v| v.as_str())
                .map(String::from);
            let session_id = args_obj
                .get("sessionId")
                .and_then(|v| v.as_str())
                .map(String::from);
            let file_a = args_obj
                .get("fileA")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let file_b = args_obj
                .get("fileB")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let source_file_arg = args_obj
                .get("sourceFile")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let result = match name {
                "analyze_test_quality" => run_analyze(&file),
                "suggest_improvements" => run_suggest(&file),
                "get_mutation_score" => run_mutation_score(&file, count),
                "analyze_with_source" => {
                    run_analyze_with_source(&test_file, source_file.as_deref())
                }
                "get_improvement_plan" => run_get_improvement_plan(if test_file.is_empty() {
                    &file
                } else {
                    &test_file
                }),
                "explain_rule" => run_explain_rule(rule_id.as_deref().unwrap_or("")),
                "iterate_improvement" => run_iterate_improvement(
                    if test_file.is_empty() {
                        &file
                    } else {
                        &test_file
                    },
                    session_id.as_deref(),
                ),
                "get_test_template" => run_get_test_template(if source_file_arg.is_empty() {
                    &file
                } else {
                    &source_file_arg
                }),
                "compare_tests" => run_compare_tests(&file_a, &file_b),
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

        // JSON-RPC 2.0: notifications (no id) MUST NOT receive a response.
        // MCP clients send "notifications/initialized" after the initialize handshake.
        if req.id.is_none() {
            continue;
        }

        let response = handle_request(&req);
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn ai_feedback_from_issues(issues: &[Issue]) -> Option<serde_json::Value> {
    let ai_rules: Vec<String> = issues
        .iter()
        .filter_map(|i| match i.rule {
            Rule::TautologicalAssertion
            | Rule::OverMocking
            | Rule::ShallowVariety
            | Rule::HappyPathOnly
            | Rule::ParrotAssertion
            | Rule::BoilerplatePadding => Some(i.rule.to_string()),
            _ => None,
        })
        .collect();
    if ai_rules.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "message": "AI-generated test smells detected; consider strengthening assertions, error paths, and input variety.",
        "rules": ai_rules,
    }))
}

fn run_analyze(file: &str) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(file);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }
    let engine = AnalysisEngine::new();
    let result = engine.analyze(path, None)?;
    let mut out = serde_json::json!({
        "filePath": result.file_path,
        "score": result.score,
        "breakdown": result.breakdown,
        "issues": result.issues,
        "stats": result.stats,
    });
    if let Some(ai) = ai_feedback_from_issues(&result.issues) {
        out["aiFeedback"] = ai;
    }
    Ok(out)
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

fn run_analyze_with_source(
    test_file: &str,
    source_file: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(test_file);
    if !path.exists() {
        anyhow::bail!("Test file not found: {}", test_file);
    }
    let engine = AnalysisEngine::new();
    let result = engine.analyze(path, None)?;
    let mut out = serde_json::json!({
        "analysis": {
            "filePath": result.file_path,
            "score": result.score,
            "breakdown": result.breakdown,
            "issues": result.issues,
            "stats": result.stats,
            "sourceFile": result.source_file,
        }
    });
    if let Some(sf) = source_file {
        let sp = Path::new(sf);
        if sp.exists() {
            let content = std::fs::read_to_string(sp).ok();
            out["sourceContent"] = serde_json::json!(content.unwrap_or_default());
            out["sourcePath"] = serde_json::json!(sf);
        }
    } else if let Some(ref src) = result.source_file {
        if src.exists() {
            let content = std::fs::read_to_string(src).ok();
            out["sourceContent"] = serde_json::json!(content.unwrap_or_default());
            out["sourcePath"] = serde_json::json!(src.to_string_lossy().to_string());
        }
    }
    if let Some(ai) = ai_feedback_from_issues(&result.issues) {
        out["analysis"]["aiFeedback"] = ai;
    }
    Ok(out)
}

fn run_get_improvement_plan(file: &str) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(file);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }
    let engine = AnalysisEngine::new();
    let result = engine.analyze(path, None)?;
    let mut plan: Vec<serde_json::Value> = result
        .issues
        .iter()
        .enumerate()
        .map(|(i, issue)| {
            let priority = match issue.severity {
                crate::Severity::Error => 1,
                crate::Severity::Warning => 2,
                crate::Severity::Info => 3,
            };
            serde_json::json!({
                "priority": priority * 1000 + i,
                "rule": issue.rule.to_string(),
                "severity": format!("{:?}", issue.severity),
                "message": issue.message,
                "suggestion": issue.suggestion,
                "location": issue.location,
            })
        })
        .collect();
    plan.sort_by(|a, b| {
        let pa: u32 = a["priority"].as_u64().unwrap_or(0) as u32;
        let pb: u32 = b["priority"].as_u64().unwrap_or(0) as u32;
        pa.cmp(&pb)
    });
    let gen = AiSuggestionGenerator::new();
    let prompt = gen.generate_prompt(&result);
    let mut out = serde_json::json!({
        "filePath": result.file_path,
        "score": result.score,
        "plan": plan,
        "prompt": prompt,
    });
    if let Some(ai) = ai_feedback_from_issues(&result.issues) {
        out["aiFeedback"] = ai;
    }
    Ok(out)
}

fn rule_description_and_examples(rule_id: &str) -> Option<(String, String, String, &'static str)> {
    let (description, example_bad, example_good, category) = match rule_id {
        "weak-assertion" => (
            "Flags assertions that are too vague (e.g. toBeTruthy, toBeDefined) instead of asserting the exact expected value.",
            "expect(result).toBeTruthy()",
            "expect(result).toBe(true) or expect(result).toEqual(expected)",
            "Assertion Quality",
        ),
        "missing-error-test" => (
            "The source can throw but there is no test that expects the error (toThrow/rejects).",
            "No it('throws...') or expect(() => fn()).toThrow()",
            "it('throws for invalid input', () => { expect(() => parse(invalid)).toThrow(ValidationError); })",
            "Error Coverage",
        ),
        "missing-boundary-test" => (
            "Boundary or edge values (0, empty, limits) from source are not covered by tests.",
            "Tests only use normal values",
            "Add expect(fn(0)).toBe(...); expect(fn('')).toBe(...);",
            "Boundary Conditions",
        ),
        "shared-state" => (
            "Shared mutable state between tests can cause order-dependent failures.",
            "let counter = 0; used in multiple tests without reset",
            "Use beforeEach to reset or const for immutable test data",
            "Test Isolation",
        ),
        "hardcoded-values" => (
            "Tests use hardcoded real-looking data (e.g. emails) instead of fixtures.",
            "expect(email).toBe('user@example.com')",
            "Use faker or test fixtures",
            "Input Variety",
        ),
        "no-assertions" => (
            "Test has no assertions so it does not verify behavior.",
            "it('works', () => { doSomething(); });",
            "it('works', () => { expect(doSomething()).toBe(expected); });",
            "Assertion Quality",
        ),
        "debug-code" => (
            "Debug code (console.log, debugger, .only) left in tests.",
            "console.log(x); or it.only(...)",
            "Remove console.log and .only before committing",
            "penalty",
        ),
        "focused-test" => (
            "Focused test (.only) will skip other tests when run.",
            "it.only('...', () => ...)",
            "it('...', () => ...)",
            "penalty",
        ),
        "flaky-pattern" => (
            "Non-deterministic or slow patterns (Date.now(), Math.random(), fetch without mock).",
            "expect(Date.now()).toBeGreaterThan(0)",
            "Use fake timers and mock fetch/axios",
            "penalty",
        ),
        "vague-test-name" => (
            "Test name does not describe the scenario or expected outcome.",
            "it('test 1', ...)",
            "it('returns 404 when user not found', ...)",
            "penalty",
        ),
        "trivial-assertion" => (
            "Assertion always passes regardless of code under test (e.g. expect(1).toBe(1)).",
            "expect(1).toBe(1)",
            "expect(actualResult).toBe(expected)",
            "Assertion Quality",
        ),
        _ => return None,
    };
    Some((
        description.to_string(),
        example_bad.to_string(),
        example_good.to_string(),
        category,
    ))
}

fn run_explain_rule(rule_id: &str) -> anyhow::Result<serde_json::Value> {
    if rule_id.is_empty() {
        anyhow::bail!("ruleId is required");
    }
    let (description, example_bad, example_good, category) = rule_description_and_examples(rule_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown rule: {}", rule_id))?;
    Ok(serde_json::json!({
        "ruleId": rule_id,
        "category": category,
        "description": description,
        "exampleBad": example_bad,
        "exampleGood": example_good,
    }))
}

fn run_iterate_improvement(
    file: &str,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(file);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }
    let engine = AnalysisEngine::new();
    let result = engine.analyze(path, None)?;
    let key = session_id.unwrap_or(file).to_string();
    let current_score = result.score.value;
    let current_issues = result.issues.len();

    let (previous_score, previous_issues, delta_summary) = {
        let mut guard = improvement_sessions()
            .lock()
            .map_err(|e| anyhow::anyhow!("lock: {}", e))?;
        let prev = guard.get(&key).cloned();
        guard.insert(
            key.clone(),
            ImprovementSession {
                score: current_score,
                issue_count: current_issues,
            },
        );
        match prev {
            Some(p) => (
                p.score,
                p.issue_count,
                format!(
                    "Score: {} -> {} ({:+}). Issues: {} -> {} ({:+})",
                    p.score,
                    current_score,
                    current_score as i32 - p.score as i32,
                    p.issue_count,
                    current_issues,
                    current_issues as i32 - p.issue_count as i32
                ),
            ),
            None => (
                current_score,
                current_issues,
                "First run for this session; no previous data to compare.".to_string(),
            ),
        }
    };

    Ok(serde_json::json!({
        "filePath": result.file_path,
        "score": result.score,
        "issues": result.issues,
        "sessionId": key,
        "previousScore": previous_score,
        "previousIssueCount": previous_issues,
        "deltaSummary": delta_summary,
    }))
}

fn run_get_test_template(source_file: &str) -> anyhow::Result<serde_json::Value> {
    let path = Path::new(source_file);
    if !path.exists() {
        anyhow::bail!("Source file not found: {}", source_file);
    }
    let content = std::fs::read_to_string(path)?;
    let mut parser = TypeScriptParser::new().map_err(|e| anyhow::anyhow!("parser: {}", e))?;
    let tree = parser
        .parse(&content)
        .map_err(|e| anyhow::anyhow!("parse: {}", e))?;
    let source_parser = SourceFileParser::new(&content);
    let exports = source_parser.extract_exports(&tree);
    let suggested_tests: Vec<serde_json::Value> = exports
        .iter()
        .filter(|e| matches!(e.kind, ExportKind::Function | ExportKind::Class))
        .map(|e| {
            serde_json::json!({
                "name": e.name,
                "placeholder": format!("expect({}(...)).toBe(expected)", e.name),
            })
        })
        .collect();
    let describe_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let it_lines: Vec<String> = suggested_tests
        .iter()
        .map(|t| {
            let name = t["name"].as_str().unwrap_or("?");
            format!(
                "it('should ...', () => {{ expect({}(...)).toBe(expected); }})",
                name
            )
        })
        .collect();
    let template = if it_lines.is_empty() {
        format!(
            "describe('{}', () => {{\n  // TODO: add tests for exported functions\n}});",
            describe_name
        )
    } else {
        format!(
            "describe('{}', () => {{\n  // TODO: add tests for exported functions\n  {};\n}});",
            describe_name,
            it_lines.join("\n  ")
        )
    };
    Ok(serde_json::json!({
        "sourcePath": source_file,
        "suggestedDescribeName": describe_name,
        "suggestedTests": suggested_tests,
        "template": template,
    }))
}

fn run_compare_tests(file_a: &str, file_b: &str) -> anyhow::Result<serde_json::Value> {
    let path_a = Path::new(file_a);
    let path_b = Path::new(file_b);
    if !path_a.exists() {
        anyhow::bail!("File not found: {}", file_a);
    }
    if !path_b.exists() {
        anyhow::bail!("File not found: {}", file_b);
    }
    let engine = AnalysisEngine::new();
    let result_a = engine.analyze(path_a, None)?;
    let result_b = engine.analyze(path_b, None)?;
    let mut issues_by_rule_a: HashMap<String, u32> = HashMap::new();
    for i in &result_a.issues {
        *issues_by_rule_a.entry(i.rule.to_string()).or_insert(0) += 1;
    }
    let mut issues_by_rule_b: HashMap<String, u32> = HashMap::new();
    for i in &result_b.issues {
        *issues_by_rule_b.entry(i.rule.to_string()).or_insert(0) += 1;
    }
    let summary = format!(
        "A: {} (score {}), {} issues. B: {} (score {}), {} issues.",
        file_a,
        result_a.score.value,
        result_a.issues.len(),
        file_b,
        result_b.score.value,
        result_b.issues.len(),
    );
    Ok(serde_json::json!({
        "fileA": {
            "path": result_a.file_path,
            "score": result_a.score,
            "issueCount": result_a.issues.len(),
            "issuesByRule": issues_by_rule_a,
        },
        "fileB": {
            "path": result_b.file_path,
            "score": result_b.score,
            "issueCount": result_b.issues.len(),
            "issuesByRule": issues_by_rule_b,
        },
        "summary": summary,
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
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "rigor");
        assert!(result["serverInfo"]["version"].is_string());
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_tools_list_returns_nine_tools() {
        let req = make_request("tools/list", None);
        let resp = handle_request(&req);

        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 9);

        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"analyze_test_quality"));
        assert!(names.contains(&"suggest_improvements"));
        assert!(names.contains(&"get_mutation_score"));
        assert!(names.contains(&"analyze_with_source"));
        assert!(names.contains(&"get_improvement_plan"));
        assert!(names.contains(&"explain_rule"));
        assert!(names.contains(&"iterate_improvement"));
        assert!(names.contains(&"get_test_template"));
        assert!(names.contains(&"compare_tests"));

        // Each tool must have inputSchema with type, properties, required
        for tool in tools {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object");
            assert!(schema["properties"].is_object());
            assert!(schema["required"].is_array());
            let required = schema["required"].as_array().unwrap();
            assert!(
                !required.is_empty(),
                "each tool must have at least one required param"
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
