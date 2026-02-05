//! Apply AI-suggested fixes: show diff and optionally write file.

use colored::Colorize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Extract the first TypeScript/JavaScript code block from AI output.
pub fn extract_code_block(output: &str) -> Option<String> {
    let start_markers = ["```typescript", "```ts", "```javascript", "```js", "```"];
    for marker in start_markers {
        if let Some(i) = output.find(marker) {
            let after = i + marker.len();
            let from = after + output[after..].find('\n').map(|j| j + 1).unwrap_or(0);
            if let Some(end_off) = output[from..].find("```") {
                let code = output[from..from + end_off].trim();
                return Some(code.to_string());
            }
        }
    }
    None
}

/// Offer to apply suggested content: show diff hint and prompt. Returns true if applied.
///
/// Note: This function reads from stdin interactively and is not easily unit-testable.
/// See extract_code_block tests below for the extractable logic.
pub fn offer_apply(path: &Path, current: &str, suggested: &str) -> io::Result<bool> {
    if current == suggested {
        return Ok(false);
    }

    println!(
        "\n{} ({} bytes) -> suggested ({} bytes)",
        "Current file".dimmed(),
        current.len(),
        suggested.len()
    );
    print!("Apply suggested changes? [y/N] ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_lowercase();

    if answer == "y" || answer == "yes" {
        fs::write(path, suggested)?;
        println!("{}", "Applied.".green());
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_code_block_typescript() {
        let output = "Here's the improved test:\n```typescript\nit('works', () => {\n  expect(1).toBe(1);\n});\n```\nDone.";
        let code = extract_code_block(output);
        assert!(code.is_some());
        let code = code.unwrap();
        assert!(code.contains("it('works'"), "got: {}", code);
        assert!(code.contains("expect(1)"));
    }

    #[test]
    fn test_extract_code_block_ts() {
        let output = "```ts\nconst x = 42;\n```";
        let code = extract_code_block(output);
        assert!(code.is_some());
        assert_eq!(code.unwrap(), "const x = 42;");
    }

    #[test]
    fn test_extract_code_block_javascript() {
        let output = "```javascript\nfunction add(a, b) { return a + b; }\n```";
        let code = extract_code_block(output);
        assert!(code.is_some());
        assert!(code.unwrap().contains("function add"));
    }

    #[test]
    fn test_extract_code_block_js() {
        let output = "```js\nconst y = 'hello';\n```";
        let code = extract_code_block(output);
        assert!(code.is_some());
        assert_eq!(code.unwrap(), "const y = 'hello';");
    }

    #[test]
    fn test_extract_code_block_generic_fence() {
        let output = "```\nsome code\n```";
        let code = extract_code_block(output);
        assert!(code.is_some());
        assert_eq!(code.unwrap(), "some code");
    }

    #[test]
    fn test_extract_code_block_no_block() {
        let output = "This is just plain text with no code fences.";
        let code = extract_code_block(output);
        assert!(code.is_none());
    }

    #[test]
    fn test_extract_code_block_unclosed() {
        let output = "```typescript\nsome code without closing";
        let code = extract_code_block(output);
        assert!(code.is_none());
    }

    #[test]
    fn test_extract_code_block_multiple_blocks() {
        // Should return the first matching block
        let output = "First:\n```ts\nfirst block\n```\nSecond:\n```ts\nsecond block\n```";
        let code = extract_code_block(output);
        assert!(code.is_some());
        assert_eq!(code.unwrap(), "first block");
    }

    #[test]
    fn test_extract_code_block_with_extra_whitespace() {
        let output = "```typescript\n\n  const x = 1;\n  const y = 2;\n\n```";
        let code = extract_code_block(output);
        assert!(code.is_some());
        let code = code.unwrap();
        assert!(code.contains("const x = 1;"));
        assert!(code.contains("const y = 2;"));
    }

    #[test]
    fn test_offer_apply_same_content_returns_false() {
        // If current == suggested, offer_apply returns Ok(false) without prompting
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.ts");
        std::fs::write(&path, "same").unwrap();

        let result = offer_apply(&path, "same", "same").unwrap();
        assert!(!result);
    }
}
