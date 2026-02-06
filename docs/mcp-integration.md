# Rigor MCP Integration

The Rigor MCP server exposes tools for test quality analysis, improvement plans, rule explanations, and more. Use it from Cursor, Continue, or Cline.

## Running the MCP server

Ensure `rigor` is on your PATH (e.g. `cargo install --path .` or use the binary from `target/release/rigor`). The server reads JSON-RPC from stdin and writes responses to stdout.

```bash
# Run once (one request per line on stdin)
rigor mcp
```

## Cursor

1. Open Cursor Settings → MCP (or edit `~/.cursor/mcp.json`).
2. Add a Rigor server entry:

```json
{
  "mcpServers": {
    "rigor": {
      "command": "rigor",
      "args": ["mcp"]
    }
  }
}
```

If `rigor` is not on PATH, use the full path to the binary, e.g. `"/path/to/rigor/target/release/rigor"`.

3. Restart Cursor or reload MCP. You can then use tools such as **analyze_test_quality**, **get_improvement_plan**, **explain_rule**, **iterate_improvement**, **get_test_template**, and **compare_tests** in the AI chat.

## Continue

1. Open Continue config (e.g. `~/.continue/config.json`).
2. Add the Rigor MCP server under `models` or in the MCP section if your Continue version supports it. Refer to [Continue’s MCP docs](https://continue.dev/docs/mcp) for the exact schema.
3. Set `command` to `rigor` and `args` to `["mcp"]` (or full path to the binary).

## Cline (formerly Claude Code)

1. In Cline settings, add an MCP server with command `rigor` and arguments `["mcp"]`.
2. Use the full path to the `rigor` binary if it is not on PATH.

## Available tools

| Tool | Description |
|------|-------------|
| **analyze_test_quality** | Analyze a test file; returns score, breakdown, and issues. |
| **suggest_improvements** | Generate an AI-oriented prompt to improve the test file. |
| **get_mutation_score** | Run mutation testing on the test’s source and return kill rate. |
| **analyze_with_source** | Analyze test file and optionally include source file content in the response. |
| **get_improvement_plan** | Prioritized action plan (issues ordered by severity) plus improvement prompt. |
| **explain_rule** | Explain a rule by id (e.g. `weak-assertion`) with good/bad examples. |
| **iterate_improvement** | Analyze and compare to the previous run for the same session (score delta, issues resolved). |
| **get_test_template** | Generate a test template for a source file from its exports. |
| **compare_tests** | Compare two test files (scores, issue counts by rule, summary). |

## Session memory (iterate_improvement)

**iterate_improvement** keeps in-memory state per session. Pass an optional `sessionId` to group multiple files or runs; if omitted, the test file path is used as the session key. The server process must stay running for the same session to be reused (e.g. one MCP server process per editor session).
