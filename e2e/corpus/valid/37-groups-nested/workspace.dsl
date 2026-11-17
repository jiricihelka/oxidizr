workspace "Nested Groups" "Groups inside groups" {

    model {
        properties {
            "structurizr.groupSeparator" "/"
        }

        group "Business Applications" {
            group "Sales" {
                crm = softwareSystem "CRM" "Pipeline"
            }

            group "Service" {
                desk = softwareSystem "Service Desk" "Tickets"
            }
        }

        crm -> desk "converts complaints to tickets in"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
