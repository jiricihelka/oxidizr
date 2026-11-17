workspace "Relationship Perspectives" "Perspectives on links" {

    model {
        portal = softwareSystem "Portal" "Client facing"
        core = softwareSystem "Core" "Business logic"

        portal -> core "calls" {
            perspectives {
                "Criticality" "Blocking path" "High"
            }
        }
    }

}
