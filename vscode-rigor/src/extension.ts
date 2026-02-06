import * as path from "path";
import { workspace } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: { subscriptions: { push(item: { dispose(): void }): void } }) {
  const enable = workspace.getConfiguration("rigor").get<boolean>("enable", true);
  if (!enable) return;

  const rigorPath = workspace.getConfiguration("rigor").get<string>("path", "rigor-lsp");
  // Use binary from PATH if config is "rigor-lsp", else use as path (absolute or relative to workspace)
  const command =
    rigorPath === "rigor-lsp"
      ? "rigor-lsp"
      : path.isAbsolute(rigorPath)
        ? rigorPath
        : path.join(workspace.workspaceFolders?.[0]?.uri.fsPath ?? "", rigorPath);

  const serverOptions: ServerOptions = {
    run: { command, args: [] },
    debug: { command, args: [] },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "typescript", pattern: "**/*.test.{ts,tsx}" },
      { scheme: "file", language: "typescript", pattern: "**/*.spec.{ts,tsx}" },
      { scheme: "file", language: "javascript", pattern: "**/*.test.{js,jsx}" },
      { scheme: "file", language: "javascript", pattern: "**/*.spec.{js,jsx}" },
      { scheme: "file", pattern: "**/*.cy.{ts,tsx,js,jsx}" },
    ],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/.rigorrc.json"),
    },
  };

  client = new LanguageClient(
    "rigor",
    "Rigor Test Quality",
    serverOptions,
    clientOptions
  );

  client.start();
  context.subscriptions.push(
    { dispose: () => client?.stop() }
  );
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
