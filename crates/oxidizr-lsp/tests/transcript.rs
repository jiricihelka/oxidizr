//! Transcript harness (§P4-LSP L-2): the REAL [`oxidizr_lsp::server::run`]
//! loop over a `Connection::memory()` pair, scripted client-side. The
//! whole exchange is pinned with `expect-test` — message streams are
//! deterministic JSON, so the transcript is the Category A contract for
//! every later item (L1's publishDiagnostics lines ride the same rail).

use expect_test::expect;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::{DidOpenTextDocument, Exit, Notification as _};
use lsp_types::request::{Initialize, Request as _, Shutdown};
use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem};
use std::fmt::Write as _;

/// Run the server loop on a paired in-memory connection; script
/// initialize → didOpen → shutdown → exit from the client end and drain
/// EVERYTHING the client received (responses AND the L1 diagnostics
/// publish) — one transcript line per message, in wire order.
#[test]
fn handshake_transcript() {
    let (server, client) = Connection::memory();
    let server_thread = std::thread::spawn(move || oxidizr_lsp::server::run(server));

    client
        .sender
        .send(
            Request::new(
                RequestId::from(1),
                Initialize::METHOD.to_owned(),
                serde_json::json!({"capabilities": {}}),
            )
            .into(),
        )
        .unwrap();
    let open = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: "file:///workspace.dsl".parse().unwrap(),
            language_id: "dsl".to_owned(),
            version: 1,
            text: "workspace {\n}".to_owned(),
        },
    };
    client
        .sender
        .send(
            Notification::new(
                DidOpenTextDocument::METHOD.to_owned(),
                serde_json::to_value(open).unwrap(),
            )
            .into(),
        )
        .unwrap();
    client
        .sender
        .send(
            Request::new(
                RequestId::from(2),
                Shutdown::METHOD.to_owned(),
                serde_json::Value::Null,
            )
            .into(),
        )
        .unwrap();
    client
        .sender
        .send(Notification::new(Exit::METHOD.to_owned(), serde_json::Value::Null).into())
        .unwrap();

    // The exit ends the loop, which drops the server side; the drain
    // then runs the queue dry and sees the disconnect.
    let mut transcript = String::new();
    while let Ok(message) = client.receiver.recv() {
        record(&mut transcript, &message);
    }
    server_thread
        .join()
        .expect("server loop ends cleanly")
        .unwrap();

    // The capabilities blob is huge, so pin only the contractive parts.
    let summary = summarize(&transcript);
    expect![["response#1 ok serverInfo=oxidizr-lsp textDocumentSync=1\n\
         publish file:///workspace.dsl v1 0 problems\n\
         response#2 ok result=null\n"]]
    .assert_eq(&summary);
}

fn record(transcript: &mut String, message: &Message) {
    let value = match message {
        Message::Response(response) => {
            // lsp-server 0.10 flattens the result/error into one `Result`.
            match &response.response_result {
                Ok(result) => serde_json::json!({ "id": response.id, "result": result }),
                Err(error) => serde_json::json!({
                    "id": response.id,
                    "error": serde_json::to_value(error).unwrap(),
                }),
            }
        }
        Message::Notification(notification) => {
            // Diagnostics publishes: keep identity + count, not payloads.
            if notification.method == "textDocument/publishDiagnostics" {
                let params: lsp_types::PublishDiagnosticsParams =
                    serde_json::from_value(notification.params.clone()).unwrap();
                serde_json::json!({
                    "method": notification.method,
                    "uri": params.uri.as_str(),
                    "version": params.version,
                    "count": params.diagnostics.len(),
                })
            } else {
                serde_json::json!({ "method": notification.method })
            }
        }
        Message::Request(request) => serde_json::json!({ "method": request.method }),
    };
    transcript.push_str(&serde_json::to_string(&value).unwrap());
    transcript.push('\n');
}

/// One line per message: ok/err responses as before, publishes as
/// `publish <uri> v<version> <n> problems` (or `publish <method>` for
/// notification kinds the harness does not know).
fn summarize(transcript: &str) -> String {
    let mut out = String::new();
    for line in transcript.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
            if method == "textDocument/publishDiagnostics" {
                let _ = writeln!(
                    out,
                    "publish {} v{} {} problems",
                    value["uri"].as_str().unwrap_or("-"),
                    value["version"].as_i64().unwrap_or(-1),
                    value["count"].as_i64().unwrap_or(-1),
                );
            } else {
                let _ = writeln!(out, "notify {method}");
            }
            continue;
        }
        let id = value["id"].as_i64().unwrap();
        if !value["error"].is_null() {
            let _ = writeln!(out, "response#{id} err {}", value["error"]["code"]);
        } else if value["result"].is_object() {
            let name = value["result"]["serverInfo"]["name"]
                .as_str()
                .unwrap_or("-");
            let sync = value["result"]["capabilities"]["textDocumentSync"].clone();
            let _ = writeln!(
                out,
                "response#{id} ok serverInfo={name} textDocumentSync={sync}"
            );
        } else {
            let _ = writeln!(out, "response#{id} ok result=null");
        }
    }
    out
}
