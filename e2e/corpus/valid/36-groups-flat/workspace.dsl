workspace "Flat Groups" "Top-level groupings" {

    model {
        group "Customer facing" {
            shop = softwareSystem "Web Shop" "Sales channel"
            portal = softwareSystem "Customer Portal" "Account self-service"
        }

        group "Back office" {
            erp = softwareSystem "ERP" "Finance and stock"
        }

        shop -> erp "sends orders to"
        portal -> erp "reads invoices from"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
