//! Transcript harness (§P4-LSP L-2): the REAL [`oxidizr_lsp::server::run`]
//! loop over a `Connection::memory()` pair, scripted client-side. The
//! whole exchange is pinned with `expect-test` — message streams are
//! deterministic JSON, so the transcript is the Category A contract
//! for every later item (L1 appends publishDiagnostics lines here).

use expect_test::expect;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::{DidOpenTextDocument, Exit, Notification as _};
use lsp_types::request::{Initialize, Request as _, Shutdown};
use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem};
use std::fmt::Write as _;
use std::str::FromStr;

/// Run the server loop on a paired in-memory connection; script
/// initialize → didOpen → shutdown → exit from the client end; return
/// every message the client received, pretty-printed as one transcript.
#[test]
fn handshake_transcript() {
    let (server, client) = Connection::memory();
    let server_thread = std::thread::spawn(move || oxidizr_lsp::server::run(&server));

    let mut transcript = String::new();
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
    let response = client.receiver.recv().expect("initialize response");
    record(&mut transcript, &response);

    let open = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: lsp_types::Uri::from_str("file:///workspace.dsl").unwrap(),
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
    let response = client.receiver.recv().expect("shutdown response");
    record(&mut transcript, &response);

    client
        .sender
        .send(Notification::new(Exit::METHOD.to_owned(), serde_json::Value::Null).into())
        .unwrap();
    server_thread
        .join()
        .expect("server loop ends cleanly")
        .unwrap();

    // The id-bearing prefix of each response line survives; the
    // capabilities blob is huge, so pin only the contractive parts.
    let summary = summarize(&transcript);
    expect![[
        "response#1 ok serverInfo=oxidizr-lsp textDocumentSync=1\nresponse#2 ok result=null\n"
    ]]
    .assert_eq(&summary);
}

fn record(transcript: &mut String, message: &Message) {
    let Message::Response(response) = message else {
        panic!("expected a response");
    };
    // lsp-server 0.10 flattens the result/error into one `Result`.
    let (result, error) = match &response.response_result {
        Ok(value) => (value.clone(), serde_json::Value::Null),
        Err(error) => (
            serde_json::Value::Null,
            serde_json::to_value(error).unwrap(),
        ),
    };
    transcript.push_str(
        &serde_json::to_string(&serde_json::json!({
            "id": response.id,
            "error": error,
            "result": result,
        }))
        .unwrap(),
    );
    transcript.push('\n');
}

/// One line per response: ok/err + the fields this milestone promises.
fn summarize(transcript: &str) -> String {
    let mut out = String::new();
    for line in transcript.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let id = value["id"].as_i64().unwrap();
        if value["error"].is_null() && value["result"].is_object() {
            let name = value["result"]["serverInfo"]["name"]
                .as_str()
                .unwrap_or("-");
            let sync = value["result"]["capabilities"]["textDocumentSync"].clone();
            let _ = writeln!(
                out,
                "response#{id} ok serverInfo={name} textDocumentSync={sync}"
            );
        } else if value["result"].is_null() && value["error"].is_null() {
            let _ = writeln!(out, "response#{id} ok result=null");
        } else {
            let _ = writeln!(out, "response#{id} err {}", value["error"]["code"]);
        }
    }
    out
}
