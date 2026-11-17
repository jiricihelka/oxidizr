workspace "Four Containers" "Hub system with several containers" {

    model {
        reporting = softwareSystem "Reporting" "BI pipeline" {
            ingest = container "Ingest" "Reads sources" "Airbyte"
            warehouse = container "Warehouse" "Columnar store" "ClickHouse"
            etl = container "Transforms" "dbt models" "dbt"
            dash = container "Dashboards" "Serving UI" "Superset"

            ingest -> warehouse "lands raw data in"
            etl -> warehouse "models data in"
            dash -> warehouse "queries"
        }
    }

    views {
        container reporting "containers" {
            include *
            autoLayout
        }
    }

}
