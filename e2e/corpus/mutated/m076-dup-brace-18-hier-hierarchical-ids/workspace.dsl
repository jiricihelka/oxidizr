workspace "Hierarchical Ids" "Cross-container references via dotted ids" {

    !identifiers hierarchical

    model {
        logistics = softwareSystem "Logistics" "Routes parcels" {
            routing = container "Routing Engine" "Computes routes" "Rust" {
                optimizer = component "Optimizer" "Solves VRP"
            }
            tracking = container "Tracking" "Parcels in flight" "Elixir"

            tracking -> routing "asks for routes from"
            routing.optimizer -> tracking "publishes legs to"
        }
    }

    views {
        container logistics "containers" {
            include *
            autoLayout
        }

        component logistics.routing "components" {
            include *
            autoLayout
        }
    }

}
}
