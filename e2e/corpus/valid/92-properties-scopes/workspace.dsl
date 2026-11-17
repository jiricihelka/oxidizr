workspace "Property Scopes" "Properties on workspace, model, element, relationship" {

    properties {
        "workspace.kind" "e2e"
    }

    model {
        properties {
            "model.domain" "insurance"
        }

        underwriter = person "Underwriter" {
            properties {
                "team" "underwriting"
            }
        }

        pricing = softwareSystem "Pricing" "Quotes" {
            properties {
                "sla.uptime" "99.9%"
            }
        }

        underwriter -> pricing "prices in" {
            properties {
                "protocol" "https"
            }
        }
    }

}
