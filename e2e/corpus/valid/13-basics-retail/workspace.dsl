workspace "Shop Floor" "Omnichannel retail core" {

    properties {
        "oxidizr.seed" "e2e-13"
        "domain" "retail"
    }

    model {
        shopper = person "Shopper" "Buys online and in store"

        storefront = softwareSystem "Storefront" "Web shop"
        pos = softwareSystem "Point of Sale" "Till software"
        inventory = softwareSystem "Inventory Core" "Stock ledger"

        shopper -> storefront "browses and buys via"
        shopper -> pos "pays at"
        storefront -> inventory "checks stock in"
        pos -> inventory "reserves stock in"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
