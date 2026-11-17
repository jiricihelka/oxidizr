workspace "Styled Systems" "Styles, properties and metadata" {

    properties {
        "oxidizr.seed" "e2e-03"
    }

    model {
        teamA = softwareSystem "Billing" "Invoicing subsystem" {
            properties {
                "team" "core"
                "sla" "gold"
            }
            tags "Critical"
        }

        teamB = softwareSystem "Reporting" "BI reports" {
            properties {
                "team" "data"
            }
        }

        teamA -> teamB "publishes events to" "Kafka" {
            properties {
                "topic" "billing.events"
            }
        }
    }

    views {
        systemContext teamA "context" {
            include *
            autoLayout
        }

        styles {
            element "Element" {
                shape Box
                background #555555
                color #ffffff
                fontSize 14
                strokeWidth 1
            }

            element "Software System" {
                background #1168bd
                stroke #0c486f
            }

            element "Critical" {
                stroke #cc0000
                strokeWidth 3
            }

            relationship "Relationship" {
                dashed true
                thickness 1
                fontSize 12
            }
        }
    }

}
