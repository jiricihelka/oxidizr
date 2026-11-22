//! Transcript harness (§P4-LSP L-2): the REAL [`oxidizr_lsp::server::run`]
//! loop over a `Connection::memory()` pair, scripted client-side. The
//! whole exchange is pinned with `expect-test` — message streams are
//! deterministic JSON, so the transcript is the Category A contract for
//! every later item (L1's publishDiagnostics lines ride the same rail).
//!
//! §P4T T-4 (gap 5) adds the per-capability transcripts (completion,
//! hover, formatting idempotence through applied edits) and the P4-V5
//! fast-edit burst stress. P4T-V3 VERIFIED 2026-11-22: the converge check
//! is WALL-CLOCK-FREE — [`run`] is a single-threaded loop over the
//! receiver and every client here scripts its FULL exchange before
//! draining, so message order is structural (the server processes the
//! queue in send order), never timing-dependent: zero sleeps and zero
//! wall-clock reads in this file.

use expect_test::expect;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::{
    DidChangeTextDocument, DidOpenTextDocument, Exit, Notification as _,
};
use lsp_types::request::{
    Completion, Formatting, HoverRequest, Initialize, Request as _, Shutdown,
};
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

/// The scripted client of the capability transcripts (§P4T T-4): the
/// WHOLE exchange goes out before anything is read — `run` is a
/// single-threaded FIFO loop over the receiver, so the recorded stream is
/// the send order, never a timing race (P4T-V3).
const URI: &str = "file:///workspace.dsl";

fn notify(client: &Connection, method: &str, params: serde_json::Value) {
    client
        .sender
        .send(Notification::new(method.to_owned(), params).into())
        .unwrap();
}

fn request(client: &Connection, id: i32, method: &str, params: serde_json::Value) {
    client
        .sender
        .send(Request::new(RequestId::from(id), method.to_owned(), params).into())
        .unwrap();
}

fn open_params(uri: &str, version: i32, text: &str) -> serde_json::Value {
    serde_json::json!({
        "textDocument": { "uri": uri, "languageId": "dsl", "version": version, "text": text },
    })
}

fn change_params(uri: &str, version: i32, text: &str) -> serde_json::Value {
    serde_json::json!({
        "textDocument": { "uri": uri, "version": version },
        "contentChanges": [{ "text": text }],
    })
}

fn at(uri: &str, line: u32, character: u32) -> serde_json::Value {
    serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character },
    })
}

fn formatting_params(uri: &str) -> serde_json::Value {
    serde_json::json!({
        "textDocument": { "uri": uri },
        "options": { "tabSize": 4, "insertSpaces": true },
    })
}

fn init(client: &Connection) {
    request(
        client,
        1,
        Initialize::METHOD,
        serde_json::json!({"capabilities": {}}),
    );
}

fn shutdown_exit(client: &Connection, id: i32) {
    request(client, id, Shutdown::METHOD, serde_json::Value::Null);
    notify(client, Exit::METHOD, serde_json::Value::Null);
}

/// Run the scripted script to `exit` and return the summarized transcript.
fn exchange(script: impl Fn(&Connection)) -> String {
    let (server, client) = Connection::memory();
    let server_thread = std::thread::spawn(move || oxidizr_lsp::server::run(server));
    script(&client);
    let mut transcript = String::new();
    while let Ok(message) = client.receiver.recv() {
        record(&mut transcript, &message);
    }
    server_thread
        .join()
        .expect("server loop ends cleanly")
        .unwrap();
    summarize(&transcript)
}

/// L-4 wire contract end-to-end: the model-scope menu answers AFTER the
/// `didOpen` publish in the documented order (FIFO), and the menu's
/// content is the engine scope (unit `completion.rs` pins the labels).
#[test]
fn completion_transcript() {
    let summary = exchange(|client| {
        init(client);
        notify(
            client,
            DidOpenTextDocument::METHOD,
            open_params(URI, 1, "workspace {\n  model {\n    "),
        );
        request(
            client,
            3,
            Completion::METHOD,
            at(URI, 2, 4), // cursor at the end of the model line
        );
        shutdown_exit(client, 4);
    });
    expect![[r"
        response#1 ok serverInfo=oxidizr-lsp textDocumentSync=1
        publish file:///workspace.dsl v1 1 problems
        response#3 ok completion=27 [!include,!docs,!adrs]
        response#4 ok result=null
    "]]
    .assert_eq(&summary);
}

/// L-5 wire contract end-to-end: identifier card (last-good parse),
/// keyword card (raw text), and the spec's NULL for nothing hoverable —
/// three requests, three replies, one wire order.
#[test]
fn hover_transcript() {
    let summary = exchange(|client| {
        init(client);
        notify(
            client,
            DidOpenTextDocument::METHOD,
            open_params(
                URI,
                1,
                "workspace {\n  model {\n    p = person \"P\"\n  }\n}\n",
            ),
        );
        request(client, 3, HoverRequest::METHOD, at(URI, 2, 4)); // `p`
        request(client, 4, HoverRequest::METHOD, at(URI, 1, 4)); // `model`
        request(client, 5, HoverRequest::METHOD, at(URI, 4, 2)); // `}` → null
        shutdown_exit(client, 6);
    });
    expect![[r#"
        response#1 ok serverInfo=oxidizr-lsp textDocumentSync=1
        publish file:///workspace.dsl v1 0 problems
        response#3 ok hover "**Person** — `Person://P`"
        response#4 ok hover "`model` — The static model: elements, relationships, deployment, archetypes."
        response#5 ok result=null
        response#6 ok result=null
    "#]].assert_eq(&summary);
}

/// L-6 + T-4 "workspace-edit idempotence": a messy buffer gets ONE whole
/// edit; after the client APPLIES it (the change sends exactly the
/// canonical print — same engine the server used) the second request is
/// the EMPTY array. The pair pins: format is idempotent through the wire.
#[test]
fn formatting_edit_idempotence_transcript() {
    const MESSY: &str = "workspace {\nmodel {\np = person \"P\"\n}\n}\n";
    let canonical = oxidizr_dsl::fmt::format(MESSY);
    let summary = exchange(|client| {
        init(client);
        notify(
            client,
            DidOpenTextDocument::METHOD,
            open_params(URI, 1, MESSY),
        );
        request(client, 3, Formatting::METHOD, formatting_params(URI));
        // The client applies the returned edit: full sync means the next
        // didChange carries exactly its newText.
        notify(
            client,
            DidChangeTextDocument::METHOD,
            change_params(URI, 2, &canonical),
        );
        request(client, 5, Formatting::METHOD, formatting_params(URI));
        shutdown_exit(client, 6);
    });
    expect![[r"
        response#1 ok serverInfo=oxidizr-lsp textDocumentSync=1
        publish file:///workspace.dsl v1 0 problems
        response#3 ok edits=1
        publish file:///workspace.dsl v2 0 problems
        response#5 ok edits=0
        response#6 ok result=null
    "]]
    .assert_eq(&summary);
}

/// P4-V5 fast-edit stress, wall-clock-free (P4T-V3): EIGHT didChanges
/// burst out before anything is read — the convergence contract is that
/// the LAST publish carries the LAST version with the LAST buffer's
/// truth (here: 0 problems, the final text parses), and every publish in
/// between is accounted for in send order. The FIFO loop makes "burst"
/// mean "queued", so no sleep or clock read is involved anywhere.
#[test]
fn fast_edit_burst_converges() {
    const BROKEN: &str = "workspace {\n  model {\n    a ~~~\n  }\n}\n";
    const VALID: &str = "workspace {\n  model {\n  }\n}\n";
    let summary = exchange(|client| {
        init(client);
        notify(
            client,
            DidOpenTextDocument::METHOD,
            open_params(URI, 1, BROKEN),
        );
        for version in 2..=9 {
            let text = if version % 2 == 0 { BROKEN } else { VALID };
            notify(
                client,
                DidChangeTextDocument::METHOD,
                change_params(URI, version, text),
            );
        }
        // v9 (odd → VALID) is the final buffer: the converge check.
        shutdown_exit(client, 10);
    });
    let publishes: Vec<&str> = summary
        .lines()
        .filter(|line| line.starts_with("publish "))
        .collect();
    assert_eq!(publishes.len(), 9, "open + 8 changes publish: {summary}");
    assert_eq!(
        *publishes.last().expect("publishes exist"),
        "publish file:///workspace.dsl v9 0 problems",
        "the LAST publish is the LAST version with the LAST buffer's truth"
    );
    for (index, line) in publishes.iter().enumerate() {
        assert!(
            line.contains(&format!(" v{} ", index + 1)),
            "publishes ride the send order: {publishes:?}"
        );
    }
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
        } else if value["result"].get("serverInfo").is_some() {
            let name = value["result"]["serverInfo"]["name"]
                .as_str()
                .unwrap_or("-");
            let sync = value["result"]["capabilities"]["textDocumentSync"].clone();
            let _ = writeln!(
                out,
                "response#{id} ok serverInfo={name} textDocumentSync={sync}"
            );
        } else if let Some(items) = value["result"].as_array() {
            // The two array answers: the completion menu (items carry
            // `label`) and the formatting edits (items carry `newText`;
            // the EMPTY array is the canonical buffer). Contents are the
            // unit suites' contract — the transcript pins shape + order.
            if items.first().is_some_and(|item| item["label"].is_string()) {
                let labels: Vec<&str> = items
                    .iter()
                    .filter_map(|item| item["label"].as_str())
                    .take(3)
                    .collect();
                let _ = writeln!(
                    out,
                    "response#{id} ok completion={} [{}]",
                    items.len(),
                    labels.join(",")
                );
            } else {
                let _ = writeln!(out, "response#{id} ok edits={}", items.len());
            }
        } else if value["result"]["contents"].is_object() {
            // Hover: the card's FIRST markdown line is its identity
            // (keyword card / model card); null hovers fall to `null`.
            let card = value["result"]["contents"]["value"]
                .as_str()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .to_owned();
            let _ = writeln!(out, "response#{id} ok hover {card:?}");
        } else {
            let _ = writeln!(out, "response#{id} ok result=null");
        }
    }
    out
}
