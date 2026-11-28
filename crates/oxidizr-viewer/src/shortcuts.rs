//! The keybinding + gesture table (headless).
//!
//! The ONE place the shell's bindings are written down: the help overlay,
//! the empty-canvas hint and the README gesture list all render
//! [`all()`], so the documented set cannot drift from what the shell does.
//! Every entry pairs a keys/gesture label with the action it performs;
//! sections group them by what the user is doing.
//!
//! The table is DATA, not code — the canvas handlers stay authoritative.
//! The audit obligation: when a binding changes in the shell, this table
//! changes in the same commit.

/// One row of the table: how you trigger it, and what happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// Keys or gesture, terse display form (`Ctrl+Z`, `right-drag`).
    pub keys: &'static str,
    /// What it does, one line, user-facing.
    pub action: &'static str,
}

/// A named group of bindings (one heading in the help overlay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    /// Group heading (`Canvas`, `Dependency arrows`, …).
    pub title: &'static str,
    /// The rows, display order.
    pub bindings: &'static [Binding],
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Canvas",
        bindings: &[
            Binding {
                keys: "wheel",
                action: "zoom at the pointer",
            },
            Binding {
                keys: "two-finger scroll",
                action: "pan (trackpad)",
            },
            Binding {
                keys: "pinch",
                action: "zoom (trackpad)",
            },
            Binding {
                keys: "left-drag on empty canvas",
                action: "pan the view",
            },
            Binding {
                keys: "middle-drag / Space+left-drag",
                action: "pan the view (secondary button)",
            },
            Binding {
                keys: "right-drag",
                action: "lasso-select a region (Shift adds)",
            },
            Binding {
                keys: "click / drag the minimap",
                action: "jump the viewport",
            },
            Binding {
                keys: "Esc",
                action: "clear the selection (closes the help first)",
            },
        ],
    },
    Section {
        title: "Selection",
        bindings: &[
            Binding {
                keys: "left-click",
                action: "select an element or arrow",
            },
            Binding {
                keys: "Shift+click",
                action: "add to / remove from the selection",
            },
            Binding {
                keys: "Shift+click while the layout dialog is open",
                action: "pin / unpin the element for the next layout run",
            },
            Binding {
                keys: "Ctrl+S",
                action: "save the workspace",
            },
        ],
    },
    Section {
        title: "Routing (per view)",
        bindings: &[
            Binding {
                keys: "double-click an arrow",
                action: "insert a waypoint into the nearest segment",
            },
            Binding {
                keys: "press a midpoint dot, drag",
                action: "insert a waypoint and route it in one move",
            },
            Binding {
                keys: "drag a waypoint handle",
                action: "move it (arrows follow elements)",
            },
            Binding {
                keys: "Alt+press a waypoint",
                action: "remove that vertex",
            },
            Binding {
                keys: "Ctrl+Shift+K",
                action: "straighten the selected arrow",
            },
            Binding {
                keys: "arrow keys",
                action: "nudge the selected element or grabbed waypoint (Shift = 10x)",
            },
            Binding {
                keys: "drag an element",
                action: "move it; enclosed arrow routes follow (Ctrl detaches)",
            },
        ],
    },
    Section {
        title: "Dependency arrows (plain .json sources)",
        bindings: &[
            Binding {
                keys: "hover an element",
                action: "port dots appear on its sides",
            },
            Binding {
                keys: "drag from a port onto another element",
                action: "create a relationship (Esc cancels)",
            },
            Binding {
                keys: "drag an endpoint handle onto another element",
                action: "reconnect that end",
            },
            Binding {
                keys: "Del / Backspace",
                action: "delete the selected relationship (undoable)",
            },
            Binding {
                keys: "details panel fields",
                action: "edit an arrow's description, technology or view label",
            },
        ],
    },
    Section {
        title: "Session",
        bindings: &[
            Binding {
                keys: "Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y)",
                action: "undo / redo across the whole session",
            },
            Binding {
                keys: "F1 or ?",
                action: "show / hide the keys & gestures overlay",
            },
            Binding {
                keys: "F2",
                action: "documentation window (workspace and selected element)",
            },
        ],
    },
];

/// Every shipped keybinding and canvas gesture, grouped for display.
#[must_use]
pub const fn all() -> &'static [Section] {
    SECTIONS
}

#[cfg(test)]
mod tests {
    use super::{Binding, all};

    #[test]
    fn sections_are_populated() {
        let sections = all();
        assert!(!sections.is_empty(), "the binding table must not be empty");
        for section in sections {
            assert!(!section.title.is_empty(), "section titles are required");
            assert!(
                !section.bindings.is_empty(),
                "section {} carries no bindings",
                section.title
            );
            for binding in section.bindings {
                assert!(!binding.keys.is_empty(), "keys are required");
                assert!(!binding.action.is_empty(), "actions are required");
            }
        }
    }

    #[test]
    fn actions_are_unique() {
        let mut seen: Vec<&str> = Vec::new();
        for section in all() {
            for binding in section.bindings {
                assert!(
                    !seen.contains(&binding.action),
                    "duplicate action: {}",
                    binding.action
                );
                seen.push(binding.action);
            }
        }
    }

    #[test]
    fn key_labels_are_unique() {
        let mut seen: Vec<&str> = Vec::new();
        for section in all() {
            for binding in section.bindings {
                assert!(
                    !seen.contains(&binding.keys),
                    "duplicate binding label: {}",
                    binding.keys
                );
                seen.push(binding.keys);
            }
        }
    }

    #[test]
    fn help_row_survives_the_canvas_hint_lookup() {
        // The empty-canvas hint looks the help line up by its help-key
        // prefix — relabelling that row must trip this test, not
        // silently drop the hint line.
        let hits: Vec<&Binding> = all()
            .iter()
            .flat_map(|section| section.bindings)
            .filter(|binding| binding.keys.starts_with("F1"))
            .collect();
        assert_eq!(hits.len(), 1, "exactly one help row for the hint");
    }
}
