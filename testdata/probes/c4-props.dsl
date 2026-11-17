// P2-M3-d probe: the property-driven C4 legs no vendored fixture reaches —
// c4plantuml.tags / .stereotypes / .elementProperties / .relationshipProperties
// / .stdlib (view properties) and plantuml.sequenceDiagram (the sequence box
// leg). Witnessed by the oracle like every other golden:
// `cargo xtask oracle gen --plantuml --c4 --fixture c4-props`.
workspace "C4 props" "Property-driven C4 legs" {
    model {
        user = person "User" "A user" {
            tags "External"
        }
        sys = softwareSystem "System" "The system" {
            web = container "Web app" "serves the UI" "Rust" {
                tags "Primary"
                properties {
                    owner "team-a"
                }
            }
            db = container "Database" "stores state" "Postgres"
            web -> db "reads and writes" "SQL" {
                tags "Async"
            }
        }
        user -> web "uses"
    }
    views {
        container sys {
            include *
            autolayout
            properties {
                c4plantuml.tags "true"
                c4plantuml.stereotypes "true"
                c4plantuml.elementProperties "true"
                c4plantuml.relationshipProperties "true"
                c4plantuml.stdlib "false"
            }
        }
        dynamic sys "Sequence" "Sequence view" {
            user -> web
            web -> db
            properties {
                plantuml.sequenceDiagram "true"
            }
        }
        styles {
            element "External" {
                color #ffffff
                background #ff0000
            }
            element "Primary" {
                color #0000ff
            }
            relationship "Async" {
                color #00ff00
            }
        }
    }
}
