workspace "Relational Ids" "Default identifier mode with named references" {


    model {
        cmdb = softwareSystem "CMDB" "Configuration items" {
            api = container "CMDB API" "CRUD for CIs" "Python"
            graph = container "CI Graph" "Relationship store" "Neo4j"

api -> ghostRelRefQz "traverses"
        }

        monitor = softwareSystem "Monitoring" "Metrics and alerts"

        monitor -> api "harvests CIs from"
    }

    views {
        container cmdb "containers" {
            include *
            autoLayout
        }
    }

}
