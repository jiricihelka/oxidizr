workspace "Groups With Metadata" "Groups carrying url and properties" {

    model {
        group "Payments Domain" {
            properties {
                "owner" "pay-team"
            

            wallet = softwareSystem "Wallet" "Stored value"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
