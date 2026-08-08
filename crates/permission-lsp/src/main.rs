use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tower_lsp_server::jsonrpc::Result;
// Glob is the conventional way to pull in the LSP type zoo.
#[allow(clippy::wildcard_imports)]
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

mod aquascope;

struct Backend {
    client: Client,
    /// Per-crate analysis, populated lazily on hover and cleared on save.
    cache: Mutex<HashMap<PathBuf, Vec<aquascope::AnalysisOutput>>>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // We re-analyze on save, so ask to be notified of saves.
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "permission-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "permission-lsp initialized")
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let pos = params.text_document_position_params.position;
        let uri = &params.text_document_position_params.text_document.uri;
        let (Some(path), Ok(line), Ok(column)) = (
            uri_to_path(uri),
            usize::try_from(pos.line),
            usize::try_from(pos.character),
        ) else {
            return Ok(None);
        };
        let Some(dir) = crate_dir(&path) else {
            return Ok(None);
        };

        let analysis = match self.analysis_for(&dir) {
            Ok(analysis) => analysis,
            // Surface setup problems (aquascope missing, wrong toolchain) in the hover.
            Err(err) => return Ok(Some(markdown_hover(format!("permission-lsp: {err}")))),
        };

        Ok(aquascope::permissions_at(&analysis, line, column)
            .map(|perms| markdown_hover(perms.to_hover_markdown())))
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        // Source changed on disk; drop cached analyses so the next hover reruns.
        self.cache.lock().unwrap().clear();
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

impl Backend {
    /// Cached analysis for a crate, running `cargo aquascope` on a cache miss.
    fn analysis_for(
        &self,
        dir: &Path,
    ) -> std::result::Result<Vec<aquascope::AnalysisOutput>, aquascope::Error> {
        if let Some(hit) = self.cache.lock().unwrap().get(dir) {
            return Ok(hit.clone());
        }
        let analysis = aquascope::run(dir)?;
        self.cache
            .lock()
            .unwrap()
            .insert(dir.to_path_buf(), analysis.clone());
        Ok(analysis)
    }
}

/// Convert a `file://` URI to a filesystem path.
fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    Some(PathBuf::from(rest))
}

/// Walk up from a file to the nearest directory containing `Cargo.toml`.
fn crate_dir(file: &Path) -> Option<PathBuf> {
    let mut dir = file.parent();
    while let Some(d) = dir {
        if d.join("Cargo.toml").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

fn markdown_hover(value: String) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: None,
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        cache: Mutex::new(HashMap::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
