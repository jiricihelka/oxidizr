workspace "Archetype Perspectives" "Perspectives declared on archetypes" {

    model {
        archetypes {
            service = container {
                tag "Service"
                perspectives {
                    "Support" "L2 supports"
                }
            }
        }

        support = softwareSystem "Support" "Helpdesk" {
            api = service "Ticket API" "REST" "Go"
        }
    }

}
