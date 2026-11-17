workspace "Nodes With Metadata" "Properties and tags on nodes" {

    !identifiers hierarchical

    model {
        ledger = softwareSystem "Ledger" "Books" {
            core = container "Core" "Engine" "Java"
        }

        deploymentEnvironment "Prod" {
            deploymentNode "zone-a" "Availability zone A" "AZ" "AZ" {
                properties {
                    "cabinet" "a-17"
                }

                host = deploymentNode "ledger-1" "Host" "Debian" "Metal,EU" {
                    coreInstance = containerInstance ledger.core {
                        properties {
                            "build" "2026.11"
                        }
                    }
                }
            }
        }
    }

}
