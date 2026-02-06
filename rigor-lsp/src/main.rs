//! Rigor LSP server: publishes test quality diagnostics on save.

use std::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
    root_uri: RwLock<Option<Url>>,
}

fn rigor_severity_to_lsp(s: rigor::Severity) -> DiagnosticSeverity {
    match s {
        rigor::Severity::Error => DiagnosticSeverity::ERROR,
        rigor::Severity::Warning => DiagnosticSeverity::WARNING,
        rigor::Severity::Info => DiagnosticSeverity::HINT,
    }
}

fn issue_to_diagnostic(issue: &rigor::Issue) -> Diagnostic {
    let start = Position::new(
        (issue.location.line.saturating_sub(1)) as u32,
        (issue.location.column.saturating_sub(1)) as u32,
    );
    let end_line = issue.location.end_line.unwrap_or(issue.location.line);
    let end_col = issue.location.end_column.unwrap_or(issue.location.column);
    let end = Position::new(
        (end_line.saturating_sub(1)) as u32,
        (end_col.saturating_sub(1)) as u32,
    );
    Diagnostic {
        range: Range::new(start, end),
        severity: Some(rigor_severity_to_lsp(issue.severity)),
        code: Some(NumberOrString::String(issue.rule.to_string())),
        code_description: None,
        source: Some("rigor".to_string()),
        message: format!("{} {}", issue.rule, issue.message),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        if let Some(ref uri) = params.root_uri {
            if let Ok(mut guard) = self.root_uri.write() {
                *guard = Some(uri.clone());
            }
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::NONE),
                        save: Some(TextDocumentSyncSaveOptions::default().into()),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "rigor-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Rigor LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("Could not resolve file path for {}", uri),
                    )
                    .await;
                return;
            }
        };

        // Only analyze test files
        let path_str = path.to_string_lossy();
        if !path_str.contains(".test.")
            && !path_str.contains(".spec.")
            && !path_str.ends_with(".cy.ts")
            && !path_str.ends_with(".cy.tsx")
        {
            return;
        }

        let work_dir = if let Ok(guard) = self.root_uri.read() {
            guard
                .as_ref()
                .and_then(|u| u.to_file_path().ok())
                .unwrap_or_else(|| path.parent().unwrap_or(path.as_path()).to_path_buf())
        } else {
            path.parent().unwrap_or(path.as_path()).to_path_buf()
        };

        match rigor::analyze_file(path.as_path(), work_dir.as_path(), None) {
            Ok(result) => {
                let diagnostics: Vec<Diagnostic> =
                    result.issues.iter().map(issue_to_diagnostic).collect();
                self.client
                    .publish_diagnostics(uri, diagnostics, None)
                    .await;
            }
            Err(e) => {
                self.client
                    .log_message(MessageType::ERROR, format!("Rigor analysis failed: {}", e))
                    .await;
                self.client.publish_diagnostics(uri, vec![], None).await;
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        root_uri: RwLock::new(None),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
