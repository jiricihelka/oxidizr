workspace "Group Crosstalk" "Relationships across groups" {

    model {
        group "Europe" {
            euShop = softwareSystem "EU Shop" "European storefront"
        }

        group "US" {
            usShop = softwareSystem "US Shop" "American storefront"
        }

euShop -> ghostRelRefQz "mirrors catalog to"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
