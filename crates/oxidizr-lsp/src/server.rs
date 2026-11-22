//! The headless server state machine (§P4-LSP L-2).
//!
//! [`Server::handle`] is a pure function of (state, inbound message) →
//! (outbound messages, keep-running): no I/O, no threads — the
//! transcript tests script it directly, and [`run`] is the only loop
//! around it (over an `lsp-server` channel pair, stdio in the binary,
//! `Connection::memory()` in the harness test).
//!
//! L-2 shipped the `initialize`/`shutdown`/`exit` handshake and
//! **full-document sync** (`didOpen`/`didChange`/`didClose`); L-3 added
//! the diagnostics republish on all three (a close ends with an EMPTY
//! publish, clearing the editor's squiggles); L-4 added
//! `textDocument/completion` over the stored buffer; L-5 added
//! `textDocument/hover` — keyword cards from the raw text, model cards
//! from the LAST SUCCESSFUL parse (`Server::last_good`, kept across a
//! failing reparse); L-6 added the two formatting requests (canonical
//! print via `oxidizr_dsl::fmt`). Requests with no implemented feature
//! answer `MethodNotFound` — replying is mandatory, an unanswered
//! request hangs the client.

use std::collections::HashMap;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Exit, Notification as _,
    PublishDiagnostics,
};
use lsp_types::request::{
    Completion, Formatting, HoverRequest, Initialize, RangeFormatting, Request as _, Shutdown,
};
use lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DocumentFormattingParams,
    DocumentRangeFormattingParams, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, OneOf, PublishDiagnosticsParams, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use oxidizr_core::Workspace;

/// One open buffer (full sync: the text is always the WHOLE document).
#[derive(Debug, Clone)]
pub struct Document {
    /// Latest full text.
    pub text: String,
    /// Client version stamp of `text` (both `didOpen` and
    /// `didChange` params carry it mandatorily — lsp-types 0.97).
    pub version: i32,
}

/// Per-connection server state: the open-document store, the hover
/// store, the configuration and the shutdown latch.
#[derive(Debug, Default)]
pub struct Server {
    /// Open buffers keyed by document URI.
    docs: HashMap<Uri, Document>,
    /// The `initializationOptions` (§P4-LSP L5, item L-7), captured on
    /// `initialize` — defaults until then, malformed options included
    /// ([`crate::config::Config`] is total).
    config: crate::config::Config,
    /// The LAST SUCCESSFUL parse per document (§P4-LSP L3): replaced on
    /// `didOpen`/`didChange` ONLY by a clean pipeline run, so a buffer
    /// mid-typing keeps hovers over the model as it last parsed; dropped
    /// on `didClose`.
    last_good: HashMap<Uri, Workspace>,
    /// Set by `shutdown`; a following `exit` then means "clean".
    shutdown: bool,
}

impl Server {
    /// A fresh server (no open documents).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The advertised server capabilities: full-document sync, the L-4
    /// completion provider (`resolveProvider` stays off — items are
    /// fully built in one pass), the L-5 hover provider and the L-6
    /// formatting pair. Diagnostics need no capability
    /// (publishDiagnostics is server-pushed); the L-7 configuration
    /// rides `initializationOptions`, not the capability set.
    #[must_use]
    pub fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            completion_provider: Some(CompletionOptions {
                // `!` opens the directive menu the moment it is typed.
                trigger_characters: Some(vec!["!".to_owned()]),
                ..CompletionOptions::default()
            }),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            // L-6: both printers answer from the stored buffer; the
            // canonical style is the formatter's OWN contract (4-space
            // indent), so the `FormattingOptions` a client sends are
            // read but deliberately not honored.
            document_formatting_provider: Some(OneOf::Left(true)),
            document_range_formatting_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        }
    }

    /// The open document for `url`, if the client has it open.
    #[must_use]
    pub fn document(&self, url: &Uri) -> Option<&Document> {
        self.docs.get(url)
    }

    /// Handle one inbound message: the outbound messages (in order)
    /// and whether the event loop must STOP (the `exit` notification
    /// is the only stopper; requests that need no open document work
    /// before any `didOpen`).
    pub fn handle(&mut self, message: &Message) -> (Vec<Message>, bool) {
        match message {
            Message::Request(request) => (self.on_request(request), false),
            Message::Notification(notification) => self.on_notification(notification),
            // The server sends no client requests (window/showMessage
            // is a notification), so Responses cannot arrive — ignore
            // defensively rather than panic on a confused client.
            Message::Response(_) => (Vec::new(), false),
        }
    }

    fn on_request(&mut self, request: &Request) -> Vec<Message> {
        match request.method.as_str() {
            Initialize::METHOD => self.initialize_response(request),
            Shutdown::METHOD => {
                self.shutdown = true;
                // The spec: shutdown's result is null; `exit` follows.
                vec![Response::new_ok(request.id.clone(), serde_json::Value::Null).into()]
            }
            Completion::METHOD => self.completion_response(request),
            HoverRequest::METHOD => self.hover_response(request),
            Formatting::METHOD => self.formatting_response(request),
            RangeFormatting::METHOD => self.range_formatting_response(request),
            other => vec![
                Response::new_err(
                    request.id.clone(),
                    ErrorCode::MethodNotFound as i32,
                    format!("unhandled method: {other}"),
                )
                .into(),
            ],
        }
    }

    fn initialize_response(&mut self, request: &Request) -> Vec<Message> {
        // §P4-LSP L5 (item L-7): capture the client's
        // `initializationOptions`. The read is TOTAL — a missing or
        // malformed options blob, or even malformed params, keeps the
        // defaults; configuration never costs the handshake.
        let options = serde_json::from_value::<InitializeParams>(request.params.clone())
            .ok()
            .and_then(|params| params.initialization_options);
        self.config = crate::config::Config::from_options(options.as_ref());
        let result = InitializeResult {
            capabilities: Self::capabilities(),
            server_info: Some(ServerInfo {
                name: "oxidizr-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        };
        // A serde failure here is a bug in OUR result type, not client
        // data — answer with an internal error, never a panic.
        let response = match serde_json::to_value(&result) {
            Ok(value) => Response::new_ok(request.id.clone(), value),
            Err(error) => Response::new_err(
                request.id.clone(),
                ErrorCode::InternalError as i32,
                format!("serialize InitializeResult: {error}"),
            ),
        };
        vec![response.into()]
    }

    /// `textDocument/completion` (§P4-LSP L2, item L-4): the stored
    /// buffer replayed through the real scope machine. A document the
    /// client never opened answers the EMPTY array — completion is
    /// opportunistic, an empty menu beats an error popup (editors race
    /// `didChange` against requests). Malformed params are the client's
    /// bug: `InvalidParams`.
    fn completion_response(&self, request: &Request) -> Vec<Message> {
        let Ok(params) = serde_json::from_value::<CompletionParams>(request.params.clone()) else {
            return vec![
                Response::new_err(
                    request.id.clone(),
                    ErrorCode::InvalidParams as i32,
                    format!("malformed {}", Completion::METHOD),
                )
                .into(),
            ];
        };
        let cursor = params.text_document_position;
        let response = self
            .document(&cursor.text_document.uri)
            .map_or_else(Vec::new, |doc| {
                crate::completion::completions(&doc.text, cursor.position)
            });
        vec![Response::new_ok(request.id.clone(), CompletionResponse::Array(response)).into()]
    }

    /// `textDocument/hover` (§P4-LSP L3, item L-5): the token under the
    /// cursor against the stored buffer and the document's last good
    /// parse. An unopened document (or a cursor with nothing hoverable)
    /// answers the spec's NULL result — hover is pure opportunism.
    /// Malformed params are the client's bug: `InvalidParams`.
    fn hover_response(&self, request: &Request) -> Vec<Message> {
        let Ok(params) = serde_json::from_value::<HoverParams>(request.params.clone()) else {
            return vec![
                Response::new_err(
                    request.id.clone(),
                    ErrorCode::InvalidParams as i32,
                    format!("malformed {}", HoverRequest::METHOD),
                )
                .into(),
            ];
        };
        let cursor = params.text_document_position_params;
        let uri = &cursor.text_document.uri;
        let response = self.document(uri).and_then(|doc| {
            crate::hover::hover(&doc.text, cursor.position, self.last_good.get(uri))
        });
        vec![Response::new_ok(request.id.clone(), response).into()]
    }

    /// `textDocument/formatting` (§P4-LSP L4, item L-6): the canonical
    /// print of the stored buffer as ONE whole-document `TextEdit`. An
    /// unopened document answers the EMPTY array (nothing to print);
    /// so does a `.json` buffer — the byte contract never sees the DSL
    /// printer ([`crate::formatting`]). Malformed params are the
    /// client's bug: `InvalidParams`.
    fn formatting_response(&self, request: &Request) -> Vec<Message> {
        let Ok(params) = serde_json::from_value::<DocumentFormattingParams>(request.params.clone())
        else {
            return vec![
                Response::new_err(
                    request.id.clone(),
                    ErrorCode::InvalidParams as i32,
                    format!("malformed {}", Formatting::METHOD),
                )
                .into(),
            ];
        };
        let uri = &params.text_document.uri;
        let edits = self.document(uri).map_or_else(Vec::new, |doc| {
            crate::formatting::full_document(uri, &doc.text)
        });
        vec![Response::new_ok(request.id.clone(), edits).into()]
    }

    /// `textDocument/rangeFormatting` (§P4-LSP L4, item L-6): the
    /// request widened to WHOLE top-level statements — canonical style
    /// is a global re-print, the choice is documented in
    /// [`crate::formatting`].
    fn range_formatting_response(&self, request: &Request) -> Vec<Message> {
        let Ok(params) =
            serde_json::from_value::<DocumentRangeFormattingParams>(request.params.clone())
        else {
            return vec![
                Response::new_err(
                    request.id.clone(),
                    ErrorCode::InvalidParams as i32,
                    format!("malformed {}", RangeFormatting::METHOD),
                )
                .into(),
            ];
        };
        let uri = &params.text_document.uri;
        let edits = self.document(uri).map_or_else(Vec::new, |doc| {
            crate::formatting::range(uri, &doc.text, params.range)
        });
        vec![Response::new_ok(request.id.clone(), edits).into()]
    }

    fn on_notification(&mut self, notification: &Notification) -> (Vec<Message>, bool) {
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let Ok(params) = serde_json::from_value(notification.params.clone()) else {
                    return (Vec::new(), false); // malformed: no URI, nothing to publish about
                };
                (self.did_open(params), false)
            }
            DidChangeTextDocument::METHOD => {
                let Ok(params) = serde_json::from_value(notification.params.clone()) else {
                    return (Vec::new(), false);
                };
                (self.did_change(params), false)
            }
            DidCloseTextDocument::METHOD => {
                let Ok(params) = serde_json::from_value(notification.params.clone()) else {
                    return (Vec::new(), false);
                };
                (self.did_close(&params), false)
            }
            Exit::METHOD => (Vec::new(), true),
            _ => (Vec::new(), false), // unknown notifications are ignored (spec)
        }
    }

    fn did_open(&mut self, params: lsp_types::DidOpenTextDocumentParams) -> Vec<Message> {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.remember(&uri, &text);
        self.docs.insert(
            uri.clone(),
            Document {
                text,
                version: params.text_document.version,
            },
        );
        self.publish(&uri)
    }

    fn did_change(&mut self, params: lsp_types::DidChangeTextDocumentParams) -> Vec<Message> {
        // FULL sync (capabilities): the content changes list carries
        // exactly one whole-document change; `last()` is the defensive
        // reading (an appended list still ends in the freshest text).
        let Some(change) = params.content_changes.into_iter().last() else {
            return Vec::new();
        };
        let uri = params.text_document.uri;
        self.remember(&uri, &change.text);
        self.docs.insert(
            uri.clone(),
            Document {
                text: change.text,
                version: params.text_document.version,
            },
        );
        self.publish(&uri)
    }

    /// Hover-store update (§P4-LSP L-5): a CLEAN pipeline parse replaces
    /// the document's last good [`Workspace`]; a failing parse keeps the
    /// previous one — a buffer mid-typing still hovers the model as it
    /// last parsed ([`crate::diagnostics::workspace`] is the judge).
    fn remember(&mut self, uri: &Uri, text: &str) {
        if let Some(workspace) = crate::diagnostics::workspace(uri, text, &self.config) {
            self.last_good.insert(uri.clone(), workspace);
        }
    }

    /// Closing drops the buffer AND clears the editor's squiggles with a
    /// final EMPTY publish (the spec's ask; version `None` — there is no
    /// buffer left to stamp).
    fn did_close(&mut self, params: &lsp_types::DidCloseTextDocumentParams) -> Vec<Message> {
        let uri = params.text_document.uri.clone();
        self.docs.remove(&uri);
        self.last_good.remove(&uri);
        let cleared = PublishDiagnosticsParams::new(uri, Vec::new(), None);
        vec![Notification::new(PublishDiagnostics::METHOD.to_owned(), cleared).into()]
    }

    /// Diagnostics for the stored buffer of `uri` (§P4-LSP L-3) as one
    /// `textDocument/publishDiagnostics` notification. Parsing is the
    /// exact CLI pipeline over the buffer — [`crate::diagnostics::collect`]
    /// never touches the filesystem, so every keystroke republish is safe.
    fn publish(&self, uri: &Uri) -> Vec<Message> {
        let Some(doc) = self.docs.get(uri) else {
            return Vec::new();
        };
        let params = PublishDiagnosticsParams::new(
            uri.clone(),
            crate::diagnostics::collect(uri, &doc.text, &self.config),
            Some(doc.version),
        );
        vec![Notification::new(PublishDiagnostics::METHOD.to_owned(), params).into()]
    }
}

/// The event loop: dispatch [`Connection::receiver`] messages through
/// [`Server::handle`] onto [`Connection::sender`] until `exit` (or the
/// client vanishes). The stdio binary and the `Connection::memory()`
/// harness both run THIS — one loop implementation, no drift.
///
/// The connection is OWNED and dropped on return — that drop closes
/// `sender`, which is what ends `lsp-server`'s writer thread; a caller
/// holding the connection while joining the io threads would hang
/// (pinned by the stdio smoke, L1).
///
/// # Errors
///
/// Only a channel send to a vanished client can fail; it ends the
/// session, reported as an error to the binary.
pub fn run(connection: Connection) -> anyhow::Result<()> {
    // Destructure to make the consumption explicit (clippy's by-value
    // lint): taking ownership IS the contract — `sender` dropping when
    // this function returns is what ends lsp-server's writer thread.
    let Connection { sender, receiver } = connection;
    let mut server = Server::new();
    while let Ok(message) = receiver.recv() {
        let (outbound, stop) = server.handle(&message);
        for message in outbound {
            sender
                .send(message)
                .map_err(|_| anyhow::anyhow!("client disconnected"))?;
        }
        if stop {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_server::{Notification, RequestId};
    use lsp_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        TextDocumentContentChangeEvent, TextDocumentIdentifier, VersionedTextDocumentIdentifier,
    };

    fn url() -> Uri {
        "file:///workspace.dsl".parse().unwrap()
    }

    fn notify(method: &str, params: serde_json::Value) -> Message {
        Notification {
            method: method.to_owned(),
            params,
        }
        .into()
    }

    fn request(method: &str) -> Message {
        Request {
            id: RequestId::from(7),
            method: method.to_owned(),
            params: serde_json::Value::Null,
        }
        .into()
    }

    fn responses(messages: &[Message]) -> Vec<&Response> {
        messages
            .iter()
            .filter_map(|message| match message {
                Message::Response(response) => Some(response),
                _ => None,
            })
            .collect()
    }

    /// The `publishDiagnostics` payloads among the outbound messages.
    fn publishes(messages: &[Message]) -> Vec<PublishDiagnosticsParams> {
        messages
            .iter()
            .filter_map(|message| match message {
                Message::Notification(notification)
                    if notification.method == PublishDiagnostics::METHOD =>
                {
                    Some(serde_json::from_value(notification.params.clone()).unwrap())
                }
                _ => None,
            })
            .collect()
    }

    /// `Message` has no `PartialEq` — assert "no outbound messages,
    /// loop continues" without needing one.
    fn assert_silent(outcome: &(Vec<Message>, bool)) {
        assert!(outcome.0.is_empty(), "expected no outbound messages");
        assert!(!outcome.1, "expected the loop to continue");
    }

    #[test]
    fn initialize_advertises_full_sync_and_server_info() {
        let mut server = Server::new();
        let (out, stop) = server.handle(&request(Initialize::METHOD));
        assert!(!stop);
        let sent = responses(&out);
        let [response] = sent.as_slice() else {
            panic!("exactly one response");
        };
        assert_eq!(response.id, RequestId::from(7));
        let Ok(value) = &response.response_result else {
            panic!("initialize must answer ok");
        };
        assert_eq!(
            value.pointer("/capabilities/textDocumentSync").unwrap(),
            &serde_json::json!(1), // TextDocumentSyncKind::FULL
            "full-document sync is the L-2 contract"
        );
        assert_eq!(
            value
                .pointer("/capabilities/completionProvider/triggerCharacters")
                .unwrap(),
            &serde_json::json!(["!"]),
            "the L-4 directive trigger is advertised"
        );
        assert_eq!(
            value.pointer("/capabilities/hoverProvider").unwrap(),
            &serde_json::json!(true),
            "the L-5 hover provider is advertised"
        );
        assert_eq!(
            value
                .pointer("/capabilities/documentFormattingProvider")
                .unwrap(),
            &serde_json::json!(true),
            "the L-6 formatting provider is advertised"
        );
        assert_eq!(
            value
                .pointer("/capabilities/documentRangeFormattingProvider")
                .unwrap(),
            &serde_json::json!(true),
            "the L-6 range-formatting provider is advertised"
        );
        assert_eq!(
            value.pointer("/serverInfo/name").unwrap(),
            &serde_json::json!("oxidizr-lsp")
        );
    }

    #[test]
    fn document_store_tracks_open_change_close() {
        let mut server = Server::new();
        let open = notify(
            DidOpenTextDocument::METHOD,
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: url(),
                    language_id: "dsl".to_owned(),
                    version: 1,
                    text: "workspace {}".to_owned(),
                },
            })
            .unwrap(),
        );
        let (out, stop) = server.handle(&open);
        assert!(!stop);
        assert_eq!(publishes(&out).len(), 1, "didOpen publishes once");
        assert_eq!(server.document(&url()).unwrap().text, "workspace {}");
        let change = notify(
            DidChangeTextDocument::METHOD,
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: url(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "workspace { model {} }".to_owned(),
                }],
            })
            .unwrap(),
        );
        let (out, _) = server.handle(&change);
        assert_eq!(publishes(&out).len(), 1, "didChange republishes");
        let doc = server.document(&url()).unwrap();
        assert_eq!(
            (doc.text.as_str(), doc.version),
            ("workspace { model {} }", 2)
        );
        let close = notify(
            DidCloseTextDocument::METHOD,
            serde_json::to_value(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: url() },
            })
            .unwrap(),
        );
        let (out, _) = server.handle(&close);
        assert!(server.document(&url()).is_none());
        let cleared = publishes(&out);
        assert_eq!(cleared.len(), 1, "didClose clears once");
        assert!(cleared[0].diagnostics.is_empty(), "clearing is EMPTY");
        assert_eq!(cleared[0].version, None, "no buffer left to stamp");
    }

    #[test]
    fn broken_buffer_publishes_real_pipeline_diagnostics() {
        // End-to-end through the state machine: the publish carries the
        // SAME errors `oxidizr validate` prints (Input::Bytes pipeline).
        let mut server = Server::new();
        let broken = "workspace {\n  model {\n    this is not dsl ~~~~\n  }\n}\n";
        let (out, _) = server.handle(&notify(
            DidOpenTextDocument::METHOD,
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: url(),
                    language_id: "dsl".to_owned(),
                    version: 3,
                    text: broken.to_owned(),
                },
            })
            .unwrap(),
        ));
        let published = publishes(&out);
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].uri, url());
        assert_eq!(
            published[0].version,
            Some(3),
            "the publish stamps the buffer version"
        );
        assert_ne!(
            published[0].diagnostics,
            Vec::<lsp_types::Diagnostic>::new()
        );
        assert!(
            published[0].diagnostics[0]
                .message
                .contains("this is not dsl")
                || published[0].diagnostics[0].message.contains("Unexpected"),
            "{:?}",
            published[0].diagnostics[0].message
        );
    }

    #[test]
    fn unknown_method_gets_method_not_found_with_the_same_id() {
        let mut server = Server::new();
        // L-6 implemented BOTH formatting requests, so the fallback
        // example is a method §P4-LSP never ships.
        let (out, stop) = server.handle(&request("textDocument/documentSymbol"));
        assert!(!stop);
        let sent = responses(&out);
        let [response] = sent.as_slice() else {
            panic!("exactly one response");
        };
        let Err(error) = &response.response_result else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, ErrorCode::MethodNotFound as i32);
        assert!(error.message.contains("textDocument/documentSymbol"));
        assert_eq!(response.id, RequestId::from(7), "clients match on the id");
    }

    /// Send one feature request (`method` + `params`) through the state
    /// machine and return its response.
    fn feature_request(server: &mut Server, method: &str, params: serde_json::Value) -> Response {
        let message: Message = Request {
            id: RequestId::from(9),
            method: method.to_owned(),
            params,
        }
        .into();
        let (out, _) = server.handle(&message);
        let sent = responses(&out);
        let [response] = sent.as_slice() else {
            panic!("exactly one response to a request");
        };
        (*response).clone()
    }

    fn completion_params(uri: Uri, line: u32, character: u32) -> serde_json::Value {
        serde_json::to_value(lsp_types::CompletionParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: lsp_types::Position::new(line, character),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
            context: None,
        })
        .unwrap()
    }

    #[test]
    fn completion_answers_from_the_stored_buffer() {
        let mut server = Server::new();
        let open = notify(
            DidOpenTextDocument::METHOD,
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: url(),
                    language_id: "dsl".to_owned(),
                    version: 1,
                    text: "workspace {\n  model {\n".to_owned(),
                },
            })
            .unwrap(),
        );
        server.handle(&open);
        let response = feature_request(
            &mut server,
            Completion::METHOD,
            completion_params(url(), 2, 0),
        );
        let Ok(value) = &response.response_result else {
            panic!("completion must answer ok");
        };
        let labels: Vec<&str> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["label"].as_str().unwrap())
            .collect();
        assert!(labels.contains(&"person"), "the Model scope: {labels:?}");
        assert!(
            !labels.contains(&"views"),
            "not the Workspace scope: {labels:?}"
        );
    }

    #[test]
    fn completion_for_an_unopened_document_is_empty() {
        // Editors race `didChange` against requests — an empty menu, not
        // an error popup, is the contract (L-4).
        let mut server = Server::new();
        let response = feature_request(
            &mut server,
            Completion::METHOD,
            completion_params(url(), 0, 0),
        );
        assert_eq!(
            response.response_result.as_ref().unwrap(),
            &serde_json::json!([])
        );
    }

    #[test]
    fn malformed_completion_params_get_invalid_params() {
        let mut server = Server::new();
        let response = feature_request(&mut server, Completion::METHOD, serde_json::json!(42));
        let Err(error) = &response.response_result else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, ErrorCode::InvalidParams as i32);
    }

    fn hover_params(uri: Uri, line: u32, character: u32) -> serde_json::Value {
        serde_json::to_value(HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: lsp_types::Position::new(line, character),
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        })
        .unwrap()
    }

    fn open_dsl(server: &mut Server, text: &str) {
        let open = notify(
            DidOpenTextDocument::METHOD,
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: url(),
                    language_id: "dsl".to_owned(),
                    version: 1,
                    text: text.to_owned(),
                },
            })
            .unwrap(),
        );
        server.handle(&open);
    }

    /// `hover` markup of an ok response (panics with the response value
    /// in hand when the contract is broken).
    fn hover_text(response: &Response) -> String {
        let Ok(value) = &response.response_result else {
            panic!("hover must answer ok, got {response:?}");
        };
        value["contents"]["value"]
            .as_str()
            .unwrap_or_else(|| panic!("markdown hover expected: {value}"))
            .to_owned()
    }

    const HOISTED: &str = "workspace {\n  model {\n    p = person \"P\"\n  }\n}\n";

    #[test]
    fn hover_answers_keyword_cards_from_the_stored_text() {
        let mut server = Server::new();
        open_dsl(&mut server, HOISTED);
        let response =
            feature_request(&mut server, HoverRequest::METHOD, hover_params(url(), 1, 4));
        let text = hover_text(&response);
        assert!(text.contains("static model"), "{text}");
    }

    #[test]
    fn hover_answers_model_cards_from_the_last_good_parse() {
        let mut server = Server::new();
        open_dsl(&mut server, HOISTED);
        // The identifier `p` (line 2, char 4) — the identifier property
        // the parser recorded, not the `person` keyword beside it.
        let response =
            feature_request(&mut server, HoverRequest::METHOD, hover_params(url(), 2, 4));
        let text = hover_text(&response);
        assert!(text.contains("Person://P"), "{text}");
    }

    #[test]
    fn hover_survives_a_broken_reparse() {
        // The last good parse is kept across a failing change — the
        // quoted `"P"` (line 2, char 16) still renders its card.
        let mut server = Server::new();
        open_dsl(&mut server, HOISTED);
        let change = notify(
            DidChangeTextDocument::METHOD,
            serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: url(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "workspace {\n  model {\n    p = person \"P\" ~~~\n  }\n}\n".to_owned(),
                }],
            })
            .unwrap(),
        );
        server.handle(&change);
        let response = feature_request(
            &mut server,
            HoverRequest::METHOD,
            hover_params(url(), 2, 16),
        );
        let text = hover_text(&response);
        assert!(text.contains("Person://P"), "{text}");
    }

    #[test]
    fn hover_for_an_unopened_document_is_null() {
        // The spec's "no hover" is JSON null, not an error (L-5).
        let mut server = Server::new();
        let response =
            feature_request(&mut server, HoverRequest::METHOD, hover_params(url(), 0, 0));
        assert_eq!(
            response.response_result.as_ref().unwrap(),
            &serde_json::Value::Null
        );
    }

    #[test]
    fn malformed_hover_params_get_invalid_params() {
        let mut server = Server::new();
        let response = feature_request(&mut server, HoverRequest::METHOD, serde_json::json!(42));
        let Err(error) = &response.response_result else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, ErrorCode::InvalidParams as i32);
    }

    /// `textDocument/formatting` (or, with `range`, `rangeFormatting`)
    /// — the `options` are the spec's minimum (`tabSize`,
    /// `insertSpaces`); the server reads but does not honor them.
    fn formatting_params(uri: Uri, range: Option<lsp_types::Range>) -> serde_json::Value {
        // The identifier is BUILT so `uri` rides by value (the `json!`
        // macro would only borrow it).
        let text_document = serde_json::to_value(TextDocumentIdentifier { uri }).unwrap();
        let mut params = serde_json::json!({
            "textDocument": text_document,
            "options": { "tabSize": 4, "insertSpaces": true },
        });
        if let Some(range) = range {
            params["range"] = serde_json::to_value(range).unwrap();
        }
        params
    }

    #[test]
    fn formatting_prints_the_stored_buffer_canonically() {
        let mut server = Server::new();
        open_dsl(&mut server, HOISTED);
        let response = feature_request(
            &mut server,
            Formatting::METHOD,
            formatting_params(url(), None),
        );
        let Ok(value) = &response.response_result else {
            panic!("formatting must answer ok");
        };
        let [edit] = value.as_array().unwrap().as_slice() else {
            panic!("one whole-buffer edit: {value}");
        };
        assert_eq!(edit["range"]["start"]["line"], serde_json::json!(0));
        let text = edit["newText"].as_str().unwrap();
        assert!(text.contains("\n    model {\n"), "{text}"); // 4-space canon
        assert!(text.ends_with("}\n"), "{text}");
    }

    #[test]
    fn range_formatting_widens_to_the_statement() {
        let mut server = Server::new();
        open_dsl(&mut server, HOISTED);
        let selection = lsp_types::Range::new(
            lsp_types::Position::new(2, 4),
            lsp_types::Position::new(2, 8),
        );
        let response = feature_request(
            &mut server,
            RangeFormatting::METHOD,
            formatting_params(url(), Some(selection)),
        );
        let Ok(value) = &response.response_result else {
            panic!("range formatting must answer ok");
        };
        let [edit] = value.as_array().unwrap().as_slice() else {
            panic!("the statement joins into one edit: {value}");
        };
        // The `p = person` line belongs to the ONE workspace statement:
        // the edit spans the whole buffer, not the requested chars.
        assert_eq!(edit["range"]["start"]["line"], serde_json::json!(0));
        assert_eq!(edit["range"]["start"]["character"], serde_json::json!(0));
    }

    #[test]
    fn formatting_an_unopened_document_is_empty() {
        let mut server = Server::new();
        let response = feature_request(
            &mut server,
            Formatting::METHOD,
            formatting_params(url(), None),
        );
        assert_eq!(
            response.response_result.as_ref().unwrap(),
            &serde_json::json!([])
        );
    }

    #[test]
    fn malformed_formatting_params_get_invalid_params() {
        let mut server = Server::new();
        let response = feature_request(&mut server, Formatting::METHOD, serde_json::json!(42));
        let Err(error) = &response.response_result else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, ErrorCode::InvalidParams as i32);
    }

    /// `initialize` with raw JSON options; asserts the handshake itself
    /// answered ok (options must NEVER cost it, §L-7).
    fn initialize_with(server: &mut Server, options: &serde_json::Value) {
        let params = serde_json::json!({
            "processId": null,
            "capabilities": {},
            "initializationOptions": options,
        });
        let init = Request {
            id: RequestId::from(1),
            method: Initialize::METHOD.to_owned(),
            params,
        };
        let (out, stop) = server.handle(&init.into());
        assert!(!stop);
        let sent = responses(&out);
        let [response] = sent.as_slice() else {
            panic!("initialize must answer exactly once");
        };
        assert!(response.response_result.is_ok(), "{response:?}");
    }

    /// `didOpen` capturing the publish the open triggers.
    fn open_capture(server: &mut Server, text: &str) -> Vec<PublishDiagnosticsParams> {
        let open = notify(
            DidOpenTextDocument::METHOD,
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: url(),
                    language_id: "dsl".to_owned(),
                    version: 1,
                    text: text.to_owned(),
                },
            })
            .unwrap(),
        );
        let (out, _) = server.handle(&open);
        publishes(&out)
    }

    /// Three distinct junk lines ⇒ three recovered errors (D1 recovery
    /// reports every broken statement).
    const DIRTY: &str = "workspace {\n  model {\n    a ~~~\n    b ~~~\n    c ~~~\n  }\n}\n";

    #[test]
    fn initialization_options_cap_the_publish() {
        let mut server = Server::new();
        initialize_with(
            &mut server,
            &serde_json::json!({ "maxNumberOfProblems": 2 }),
        );
        let published = open_capture(&mut server, DIRTY);
        assert_eq!(
            published[0].diagnostics.len(),
            2,
            "the cap truncates the publish (L-7)"
        );
    }

    #[test]
    fn malformed_options_keep_defaults_and_the_handshake() {
        // A bare scalar where the options object belongs: initialize
        // still answers ok and the FULL cap (defaults) publishes.
        let mut server = Server::new();
        initialize_with(&mut server, &serde_json::json!(42));
        let published = open_capture(&mut server, DIRTY);
        assert_eq!(published[0].diagnostics.len(), 3, "{DIRTY:?}");
    }

    #[test]
    fn shutdown_then_exit_stops() {
        let mut server = Server::new();
        let (out, stop) = server.handle(&request(Shutdown::METHOD));
        assert!(!stop, "shutdown alone keeps the loop alive for exit");
        let sent = responses(&out);
        assert!(
            matches!(&sent[0].response_result, Ok(serde_json::Value::Null)),
            "shutdown's result is null"
        );
        let (out, stop) = server.handle(&notify(Exit::METHOD, serde_json::Value::Null));
        assert!(stop, "exit ends the event loop");
        assert!(out.is_empty(), "exit is a notification — no reply");
    }

    #[test]
    fn unknown_notifications_and_responses_are_ignored() {
        let mut server = Server::new();
        assert_silent(&server.handle(&notify("$/debugPrompt", serde_json::Value::Null)));
        let response: Message =
            Response::new_ok(RequestId::from(1), serde_json::Value::Null).into();
        assert_silent(&server.handle(&response));
    }

    #[test]
    fn malformed_did_open_is_ignored_not_fatal() {
        let mut server = Server::new();
        assert_silent(&server.handle(&notify(DidOpenTextDocument::METHOD, serde_json::json!(42))));
        assert!(server.docs.is_empty());
    }
}
