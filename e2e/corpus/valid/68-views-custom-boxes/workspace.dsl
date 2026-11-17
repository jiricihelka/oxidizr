workspace "Custom View" "Custom view over landscape boxes" {

    model {
        sales = element "Sales" "Domain" "Selling" "Domain"
        fulfil = element "Fulfilment" "Domain" "Delivery" "Domain"

        shop = softwareSystem "Shop" "Direct sales"
        wms = softwareSystem "WMS" "Warehouse"

        shop -> wms "sends orders to"
    }

    views {
        custom "domains" "Domain View" "Boxes side by side" {
            title "Domains"
            description "Just the boxes"

            include sales fulfil

            autolayout
        }
    }

}
