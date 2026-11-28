//! The adoption gate for the `dagre` engine: **byte-identical output for the
//! same input** — the crate-wide determinism contract, applied to an
//! externally supplied pipeline.
//!
//! Two independent axes are proven, because they fail for different reasons:
//!
//! * **In-process repeats.** The engine mints internal dummy-vertex names
//!   from a counter that is global and monotonic per process, so the second
//!   layout in a process sees different internal names than the first. If any
//!   of that bookkeeping reached the geometry, repeat 2 would differ — this
//!   is the axis a single-process test cannot see through.
//! * **Fresh processes.** The engine's graph keys live in hash maps whose
//!   iteration order depends on a per-process random seed, and `HashMap`
//!   order can only change between processes. Re-running in one process is
//!   blind to it, so the battery runs again in ten spawned children of this
//!   very test binary and every report must match, byte for byte.
//!
//! The battery is the shared corpus (chains, diamonds, cycles, fans,
//! disconnected pieces, parallel edges, self-loops, degenerate sizes, ids
//! needing no escaping, a C4-shaped component view, two large pseudo-random
//! graphs) swept over all four rank directions plus a tight-separation and a
//! vertices-off profile. Coordinates are compared as bit patterns, so `±0.0`
//! and formatting noise cannot mask a difference.

#![cfg(feature = "dagre")]

mod common;

use std::io::Read;
use std::process::{Command, Stdio};

use oxidizr_layout::Dagre;

/// Set in the child so it prints its report instead of spawning children.
const CHILD_ENV: &str = "OXIDZR_DAGRE_DETERMINISM_CHILD";

/// This test's name — the child is asked to run exactly this test.
const TEST_NAME: &str = "dagre_output_is_byte_identical_across_repeats_and_processes";

/// Marks the child's report lines on stdout (the harness prints its own
/// progress lines to the same stream).
const PREFIX: &str = "OXZ-DIGEST ";

/// How many fresh processes the battery runs in (`V1`: ten).
const PROCESSES: usize = 10;

/// How many times the battery runs inside this process (the internal-name
/// counter drifts between them).
const REPEATS: usize = 3;

#[test]
fn dagre_output_is_byte_identical_across_repeats_and_processes() {
    let engine = Dagre::new();
    if std::env::var_os(CHILD_ENV).is_some() {
        for line in common::report(&engine, "dagre").lines() {
            println!("{PREFIX}{line}");
        }
        return;
    }

    let expected = common::report(&engine, "dagre");
    for repeat in 2..=REPEATS {
        let again = common::report(&engine, "dagre");
        assert_eq!(
            first_difference(&expected, &again),
            None,
            "in-process repeat {repeat} of the battery differs from the first run"
        );
    }

    let mut reports = spawn_reports(PROCESSES);
    for (index, report) in reports.drain(..).enumerate() {
        assert_eq!(
            first_difference(&expected, &report),
            None,
            "child process {} of {PROCESSES} differs from this process",
            index + 1
        );
    }
    // Sanity: a report that is empty (a silently failing child) would compare
    // equal to another empty one, so pin its shape.
    let variants: usize = common::corpus()
        .iter()
        .map(|fixture| common::variants(fixture).len())
        .sum();
    let lines = expected
        .lines()
        .filter(|line| !line.starts_with('#'))
        .count();
    assert_eq!(lines, variants, "the battery must cover every variant");
    println!(
        "dagre determinism: {variants} variants x {REPEATS} in-process runs + \
         {PROCESSES} processes, all identical (digest {})",
        common::digest(&expected)
    );
}

/// Runs this test binary in `count` fresh processes **concurrently** (the
/// proof is per-process; the wall clock is not the point) and returns one
/// report per child, in spawn order. Reports stay far below the pipe buffer,
/// so reading only after every spawn cannot deadlock.
fn spawn_reports(count: usize) -> Vec<String> {
    let exe = std::env::current_exe().expect("test binary path");
    let mut children = Vec::with_capacity(count);
    for child in 1..=count {
        let spawned = Command::new(&exe)
            .arg(TEST_NAME)
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD_ENV, child.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn a determinism child");
        children.push((child, spawned));
    }
    children
        .into_iter()
        .map(|(child, mut spawned)| {
            let mut out = String::new();
            spawned
                .stdout
                .as_mut()
                .expect("piped stdout")
                .read_to_string(&mut out)
                .expect("read the child's report");
            let status = spawned.wait().expect("wait for the child");
            assert!(status.success(), "child {child} exited with {status}");
            out.lines()
                .filter_map(|line| line.strip_prefix(PREFIX))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

/// The first line the two reports disagree on (`None` when identical) —
/// reported instead of a full-text diff, because a report is megabytes.
fn first_difference(expected: &str, other: &str) -> Option<(usize, String, String)> {
    let mut expected_lines = expected.lines().peekable();
    let mut other_lines = other.lines().peekable();
    let mut index = 0_usize;
    loop {
        match (expected_lines.next(), other_lines.next()) {
            (None, None) => return None,
            (one, other_line) if one == other_line => index += 1,
            (one, other_line) => {
                return Some((
                    index,
                    one.unwrap_or("<end>").to_owned(),
                    other_line.unwrap_or("<end>").to_owned(),
                ));
            }
        }
    }
}

/// The same proof for the built-in engine, so a regression in the engine
/// framework (input order, encoding) cannot hide behind a green `dagre` run:
/// `Layered` must be repeat- and process-stable too.
#[test]
fn layered_output_is_byte_identical_across_repeats_and_processes() {
    let engine = oxidizr_layout::Layered;
    if std::env::var_os("OXIDZR_LAYERED_DETERMINISM_CHILD").is_some() {
        for line in common::report(&engine, "layered").lines() {
            println!("{PREFIX}{line}");
        }
        return;
    }
    let expected = common::report(&engine, "layered");
    let again = common::report(&engine, "layered");
    assert_eq!(
        first_difference(&expected, &again),
        None,
        "the built-in engine is not repeat-stable"
    );
    // One child is enough to prove the axis here (the engine is hash-order
    // free by construction); `dagre` above carries the ten-process bar.
    let exe = std::env::current_exe().expect("test binary path");
    let output = Command::new(exe)
        .arg("layered_output_is_byte_identical_across_repeats_and_processes")
        .arg("--exact")
        .arg("--nocapture")
        .env("OXIDZR_LAYERED_DETERMINISM_CHILD", "1")
        .stderr(Stdio::piped())
        .output()
        .expect("run the layered child");
    assert!(output.status.success(), "layered child failed");
    let child = String::from_utf8_lossy(&output.stdout);
    let child = child
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        first_difference(&expected, &child),
        None,
        "the built-in engine differs between processes"
    );
}
