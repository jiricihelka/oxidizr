workspace "Perspectives Deep" "Container and component perspectives" {

    !identifiers hierarchical

    model {
        data = softwareSystem "Data" "Platform" {
            lake = container "Lake" "Objects" "S3" {
                perspectives {
                    "Residency" "EU only"
                }
                ingest = component "Ingest" "Lands files" {
                    perspectives {
                        "Throughput" "10 GB/h" "Medium"
                    }
                }
            }
        }
    }

}
