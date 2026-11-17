workspace "Dotted Targets" "Relationships addressed by dotted id" {

    !identifiers hierarchical

    model {
        analytics = softwareSystem "Analytics" "Data platform" {
            lake = container "Data Lake" "Object storage" "S3"
            query = container "Query Engine" "Interactive SQL" "Trino"
        }

        ops = softwareSystem "Ops Portal" "Internal tools"

        ops -> analytics.lake "drops logs into"
        analytics.query -> analytics.lake "scans"
    }

    views {
        container analytics "containers" {
            include *
            autoLayout
        }
    }

}
