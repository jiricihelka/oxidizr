workspace "Deep Groups" "Three levels of grouping" {

    model {
        properties {
            "structurizr.groupSeparator" "/"
        }

        group "Group Technology" {
            group "Platforms" {
                group "Containers" {
                    registry = softwareSystem "Image Registry" "OCI artifacts"
                    fleet = softwareSystem "Fleet Manager" "Cluster lifecycle"
                }
            }
        }

        registry -> fleet "delivers images to"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
