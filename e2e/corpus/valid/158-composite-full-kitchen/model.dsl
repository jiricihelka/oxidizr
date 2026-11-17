archetypes {
    service = container {
        tags "Service"
    }
}

ops = person "Dispatcher" "Plans routes"

platform = softwareSystem "${SYSTEM} Platform" "Routing and tracking" {
    properties {
        "domain" "logistics"
    }

    routing = service "Routing Engine" "Solves VRP" "Rust" {
        solver = component "Solver" "Optimiser"
        cache = component "Cache" "Distance matrix"

        solver -> cache "asks"
    }
    tracking = service "Tracking" "Parcels in flight" "Elixir"

    ops -> routing "plans in"
    routing -> tracking "publishes legs to"
}
