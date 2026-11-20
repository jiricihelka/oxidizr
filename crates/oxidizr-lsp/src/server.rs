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
//! publish, clearing the editor's squiggles). Requests with no
//! implemented feature answer `MethodNotFound` (L2 completion, L3 hover,
//! L4 formatting are open) — replying is mandatory, an unanswered
//! request hangs the client.

use std::collections::HashMap;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Exit, Notification as _,
    PublishDiagnostics,
};
use lsp_types::request::{Initialize, Request as _, Shutdown};
use lsp_types::{
    InitializeResult, PublishDiagnosticsParams, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};

/// One open buffer (full sync: the text is always the WHOLE document).
#[derive(Debug, Clone)]
pub struct Document {
    /// Latest full text.
    pub text: String,
    /// Client version stamp of [`text`] (both `didOpen` and
    /// `didChange` params carry it mandatorily — lsp-types 0.97).
    pub version: i32,
}

/// Per-connection server state: the open-document store and the
/// shutdown latch. Feature state joins per item (L1 last-good parses).
#[derive(Debug, Default)]
pub struct Server {
    /// Open buffers keyed by document URI.
    docs: HashMap<Uri, Document>,
    /// Set by `shutdown`; a following `exit` then means "clean".
    shutdown: bool,
}

impl Server {
    /// A fresh server (no open documents).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The advertised server capabilities: full-document sync ONLY —
    /// diagnostics need no capability (publishDiagnostics is
    /// server-pushed). Later items add their providers here (L2
    /// `completion_provider`, L3 `hover_provider`, L4
    /// `document_formatting_provider`).
    #[must_use]
    pub fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
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
            Initialize::METHOD => Self::initialize_response(request),
            Shutdown::METHOD => {
                self.shutdown = true;
                // The spec: shutdown's result is null; `exit` follows.
                vec![Response::new_ok(request.id.clone(), serde_json::Value::Null).into()]
            }
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

    fn initialize_response(request: &Request) -> Vec<Message> {
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
        self.docs.insert(
            uri.clone(),
            Document {
                text: params.text_document.text,
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
        self.docs.insert(
            uri.clone(),
            Document {
                text: change.text,
                version: params.text_document.version,
            },
        );
        self.publish(&uri)
    }

    /// Closing drops the buffer AND clears the editor's squiggles with a
    /// final EMPTY publish (the spec's ask; version `None` — there is no
    /// buffer left to stamp).
    fn did_close(&mut self, params: &lsp_types::DidCloseTextDocumentParams) -> Vec<Message> {
        let uri = params.text_document.uri.clone();
        self.docs.remove(&uri);
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
            crate::diagnostics::collect(uri, &doc.text),
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
        let (out, stop) = server.handle(&request("textDocument/completion"));
        assert!(!stop);
        let sent = responses(&out);
        let [response] = sent.as_slice() else {
            panic!("exactly one response");
        };
        let Err(error) = &response.response_result else {
            panic!("expected an error response");
        };
        assert_eq!(error.code, ErrorCode::MethodNotFound as i32);
        assert!(error.message.contains("textDocument/completion"));
        assert_eq!(response.id, RequestId::from(7), "clients match on the id");
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
