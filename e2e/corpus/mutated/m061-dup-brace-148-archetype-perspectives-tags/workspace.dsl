workspace "Archetype Perspectives" "Perspectives and metadata via archetypes" {

    model {
        archetypes {
            critical = softwareSystem {
                tags "Critical"
                perspectives {
                    "Impact" "Outage stops revenue" "High"
                }
            }
            }
        }

        payments = critical "Payments" "Card acquiring"
        search = softwareSystem "Search" "Find stuff"

        payments -> search "indexes via"
    }

}
