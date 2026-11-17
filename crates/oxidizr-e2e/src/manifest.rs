//! The blessable sidecar: `workspace.e2e.json` per corpus entry
//! (P1T-D3). Hand-rolled over `serde_json` (the repo has no serde derives —
//! same rule as the JSON layer, and the format is small enough to own).
//!
//! Asserted per command: **exit class + sorted-deduped OXZ code set +
//! structural counts**. NEVER bytes, NEVER message text. A section absent
//! from the manifest means the command does not apply to the entry (the
//! runner checks BOTH ways: unregistered drift fails, stale sections fail).

use std::fs;
use std::path::Path;

use serde_json::{Value, map::Map};

/// Sidecar file name inside the entry directory.
pub const FILE_NAME: &str = "workspace.e2e.json";

/// Exit class as recorded (`P1-D8` codes: ok=0, error=1, usage=2, io=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Success.
    Ok,
    /// Diagnostics with an error (exit 1).
    Error,
    /// Usage error (exit 2).
    Usage,
    /// I/O / input-policy failure (exit 3).
    Io,
}

impl Exit {
    /// Map the pipeline class to its recorded name.
    #[must_use]
    pub fn from_class(class: oxidizr_api::ExitClass) -> Self {
        use oxidizr_api::ExitClass;
        match class {
            ExitClass::Ok => Exit::Ok,
            ExitClass::Diagnostics => Exit::Error,
            ExitClass::Usage => Exit::Usage,
            ExitClass::Io => Exit::Io,
        }
    }

    #[must_use]
    fn as_str(self) -> &'static str {
        match self {
            Exit::Ok => "ok",
            Exit::Error => "error",
            Exit::Usage => "usage",
            Exit::Io => "io",
        }
    }

    fn parse(s: &str, at: &str) -> Result<Self, String> {
        match s {
            "ok" => Ok(Exit::Ok),
            "error" => Ok(Exit::Error),
            "usage" => Ok(Exit::Usage),
            "io" => Ok(Exit::Io),
            other => Err(format!("{at}: unknown exit class `{other}`")),
        }
    }
}

/// Structural counts of an exported workspace (volatile fields never enter:
/// the counters are topology, `structurizr.dsl.identifier` UUIDs and
/// `lastModified*` are not read — P1T-D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// All model elements (people, systems, containers, components).
    pub elements: usize,
    /// Model relationships.
    pub relationships: usize,
    /// Views of every kind.
    pub views: usize,
}

/// One command's assertion block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReport {
    /// Expected exit class.
    pub exit: Exit,
    /// Sorted, deduplicated OXZ codes of every diagnostic (empty = none).
    pub codes: Vec<u16>,
    /// Structural counts, present when the command exported a workspace
    /// successfully (`dsl2json` with exit `ok`).
    pub counts: Option<Counts>,
}

/// `fmt` block: exit + whether formatting is a fixed point (P1T-D6's
/// idempotence, cheap enough to keep in the quick tier).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmtReport {
    /// Expected exit class.
    pub exit: Exit,
    /// `fmt(fmt(x)) == fmt(x)`.
    pub idempotent: bool,
}

/// The whole sidecar. `Option` sections = command not applicable to the entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    /// `dsl2json` block.
    pub dsl2json: Option<CommandReport>,
    /// `json2json` block (re-export of the `dsl2json` output).
    pub json2json: Option<CommandReport>,
    /// `validate` block.
    pub validate: Option<CommandReport>,
    /// `fmt` block.
    pub fmt: Option<FmtReport>,
}

impl Manifest {
    /// Serialize to the sidecar JSON value (stable key order: struct order
    /// under `preserve_order`).
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        if let Some(r) = &self.dsl2json {
            map.insert("dsl2json".into(), command_to_value(r));
        }
        if let Some(r) = &self.json2json {
            map.insert("json2json".into(), command_to_value(r));
        }
        if let Some(r) = &self.validate {
            map.insert("validate".into(), command_to_value(r));
        }
        if let Some(r) = &self.fmt {
            let mut m = Map::new();
            m.insert("exit".into(), Value::String(r.exit.as_str().into()));
            m.insert("idempotent".into(), Value::Bool(r.idempotent));
            map.insert("fmt".into(), Value::Object(m));
        }
        Value::Object(map)
    }

    /// Parse a sidecar value (shape-checked with `at` in every message).
    /// Unknown keys fail — a typo'd section must not silently skip checks.
    ///
    /// # Errors
    ///
    /// Any shape violation (non-object, unknown section, bad exit class,
    /// missing field) with the `at` path locating the offending node.
    pub fn from_value(value: &Value, at: &str) -> Result<Self, String> {
        let obj = value
            .as_object()
            .ok_or_else(|| format!("{at}: manifest must be a JSON object"))?;
        let mut out = Manifest::default();
        for (key, val) in obj {
            match key.as_str() {
                "dsl2json" => {
                    out.dsl2json = Some(command_from_value(val, &format!("{at}.dsl2json"))?);
                }
                "json2json" => {
                    out.json2json = Some(command_from_value(val, &format!("{at}.json2json"))?);
                }
                "validate" => {
                    out.validate = Some(command_from_value(val, &format!("{at}.validate"))?);
                }
                "fmt" => {
                    let f = val
                        .as_object()
                        .ok_or_else(|| format!("{at}.fmt: must be an object"))?;
                    let exit = Exit::parse(
                        exit_str(f.get("exit"), &format!("{at}.fmt"))?,
                        &format!("{at}.fmt"),
                    )?;
                    let idempotent = f
                        .get("idempotent")
                        .and_then(Value::as_bool)
                        .ok_or_else(|| format!("{at}.fmt.idempotent: missing bool"))?;
                    out.fmt = Some(FmtReport { exit, idempotent });
                }
                other => return Err(format!("{at}: unknown section `{other}`")),
            }
        }
        Ok(out)
    }

    /// Read a sidecar file.
    ///
    /// # Errors
    ///
    /// File read failure, invalid JSON, or a shape violation via
    /// [`Manifest::from_value`].
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| format!("{}: invalid JSON: {e}", path.display()))?;
        Self::from_value(&value, &path.display().to_string())
    }

    /// Write a sidecar file (pretty JSON, trailing newline).
    ///
    /// # Errors
    ///
    /// Serialization or file write failure.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let mut text = serde_json::to_string_pretty(&self.to_value())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        text.push('\n');
        fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Human-readable per-command diff against an observed manifest (the
    /// failure text a check produces; empty = identical).
    #[must_use]
    pub fn diff(&self, observed: &Self) -> Vec<String> {
        let mut fails = Vec::new();
        for (name, exp, obs) in [
            ("dsl2json", &self.dsl2json, &observed.dsl2json),
            ("json2json", &self.json2json, &observed.json2json),
            ("validate", &self.validate, &observed.validate),
        ] {
            match (exp, obs) {
                (None, None) => {}
                (None, Some(o)) => fails.push(format!(
                    "{name}: command became applicable (unregistered drift); expected no section, got exit {} codes {:?}",
                    o.exit.as_str(),
                    o.codes
                )),
                (Some(_), None) => {
                    fails.push(format!("{name}: stale section — command no longer applies"));
                }
                (Some(e), Some(o)) => {
                    if e.exit != o.exit {
                        fails.push(format!(
                            "{name}: exit {} != observed {}",
                            e.exit.as_str(),
                            o.exit.as_str()
                        ));
                    }
                    if e.codes != o.codes {
                        fails.push(format!(
                            "{name}: codes {:?} != observed {:?}",
                            e.codes, o.codes
                        ));
                    }
                    if e.counts != o.counts {
                        fails.push(format!(
                            "{name}: counts {:?} != observed {:?}",
                            e.counts, o.counts
                        ));
                    }
                }
            }
        }
        match (&self.fmt, &observed.fmt) {
            (None, None) => {}
            (None, Some(o)) => fails.push(format!(
                "fmt: command became applicable (unregistered drift); expected no section, got exit {}",
                o.exit.as_str()
            )),
            (Some(_), None) => fails.push("fmt: stale section — command no longer applies".into()),
            (Some(e), Some(o)) => {
                if e.exit != o.exit {
                    fails.push(format!(
                        "fmt: exit {} != observed {}",
                        e.exit.as_str(),
                        o.exit.as_str()
                    ));
                }
                if e.idempotent != o.idempotent {
                    fails.push(format!(
                        "fmt: idempotent {} != observed {}",
                        e.idempotent, o.idempotent
                    ));
                }
            }
        }
        fails
    }
}

fn command_to_value(r: &CommandReport) -> Value {
    let mut m = Map::new();
    m.insert("exit".into(), Value::String(r.exit.as_str().into()));
    m.insert(
        "codes".into(),
        Value::Array(r.codes.iter().map(|c| Value::from(*c)).collect()),
    );
    match r.counts {
        Some(c) => {
            let mut cm = Map::new();
            cm.insert("elements".into(), Value::from(c.elements));
            cm.insert("relationships".into(), Value::from(c.relationships));
            cm.insert("views".into(), Value::from(c.views));
            m.insert("counts".into(), Value::Object(cm));
        }
        None => {
            m.insert("counts".into(), Value::Null);
        }
    }
    Value::Object(m)
}

fn command_from_value(value: &Value, at: &str) -> Result<CommandReport, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{at}: must be an object"))?;
    let exit = Exit::parse(exit_str(obj.get("exit"), at)?, at)?;
    let codes = obj
        .get("codes")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{at}.codes: missing array"))?
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or_else(|| format!("{at}.codes: not a code number: {v}"))
        })
        .collect::<Result<Vec<u16>, String>>()?;
    let counts = match obj.get("counts") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let c = v
                .as_object()
                .ok_or_else(|| format!("{at}.counts: must be an object or null"))?;
            let get = |k: &str| -> Result<usize, String> {
                c.get(k)
                    .and_then(Value::as_u64)
                    .map(usize::try_from)
                    .transpose()
                    .map_err(|_| format!("{at}.counts.{k}: not a number"))?
                    .ok_or_else(|| format!("{at}.counts.{k}: missing"))
            };
            Some(Counts {
                elements: get("elements")?,
                relationships: get("relationships")?,
                views: get("views")?,
            })
        }
    };
    Ok(CommandReport {
        exit,
        codes,
        counts,
    })
}

fn exit_str<'a>(value: Option<&'a Value>, at: &str) -> Result<&'a str, String> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{at}.exit: missing string"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_stable() {
        let m = Manifest {
            dsl2json: Some(CommandReport {
                exit: Exit::Ok,
                codes: vec![],
                counts: Some(Counts {
                    elements: 3,
                    relationships: 2,
                    views: 1,
                }),
            }),
            validate: Some(CommandReport {
                exit: Exit::Ok,
                codes: vec![],
                counts: None,
            }),
            fmt: Some(FmtReport {
                exit: Exit::Ok,
                idempotent: true,
            }),
            json2json: None,
        };
        let v = m.to_value();
        assert_eq!(Manifest::from_value(&v, "t").unwrap(), m);
        // Key order is deterministic (dsl2json, validate, fmt):
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["dsl2json", "validate", "fmt"]);
    }

    #[test]
    fn unknown_section_fails() {
        let v: Value = serde_json::from_str(r#"{ "dsl2jon": { } }"#).unwrap();
        assert!(
            Manifest::from_value(&v, "t")
                .unwrap_err()
                .contains("dsl2jon")
        );
    }

    #[test]
    fn diff_detects_drift_both_ways() {
        let expected = Manifest {
            dsl2json: Some(CommandReport {
                exit: Exit::Ok,
                codes: vec![1],
                counts: None,
            }),
            json2json: Some(CommandReport {
                exit: Exit::Ok,
                codes: vec![],
                counts: None,
            }),
            validate: None,
            fmt: None,
        };
        let observed = Manifest {
            dsl2json: Some(CommandReport {
                exit: Exit::Error,
                codes: vec![1, 2],
                counts: None,
            }),
            json2json: None,
            validate: Some(CommandReport {
                exit: Exit::Ok,
                codes: vec![],
                counts: None,
            }),
            fmt: None,
        };
        let d = expected.diff(&observed);
        assert_eq!(d.len(), 4, "{d:?}"); // exit, codes, stale json2json, unregistered validate
    }
}
