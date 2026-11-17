workspace "Ledger Stack" "System with two containers" {

    model {
        ledger = softwareSystem "Ledger" "Double-entry bookkeeping" {
            service = container "Ledger Service" "Posts and queries entries" "Go"
            store = container "Ledger DB" "Durable journal" "PostgreSQL"

            service -> store "reads and writes"
        }
    }

    views {
        container ledger "containers" {
            include *
            autoLayout
        
    }

}
