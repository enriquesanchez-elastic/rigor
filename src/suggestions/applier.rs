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
            let from = after
                + output[after..]
                    .find('\n')
                    .map(|j| j + 1)
                    .unwrap_or(0);
            if let Some(end_off) = output[from..].find("```") {
                let code = output[from..from + end_off].trim();
                return Some(code.to_string());
            }
        }
    }
    None
}

/// Offer to apply suggested content: show diff hint and prompt. Returns true if applied.
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
