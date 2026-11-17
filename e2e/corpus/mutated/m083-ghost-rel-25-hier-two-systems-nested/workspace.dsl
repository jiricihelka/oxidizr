workspace "Adjacent Stacks" "Two systems whose containers talk" {

    model {
        orders = softwareSystem "Orders" "Order lifecycle" {
            ordersApi = container "Orders API" "Order CRUD" "Node.js"
        }

        fulfil = softwareSystem "Fulfilment" "Picks and packs" {
            wms = container "WMS Connector" "Talks to the warehouse" "Java"
        }

ordersApi -> ghostRelRefQz "pushes orders to"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
