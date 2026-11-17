workspace "Two Environments" "Dev and Prod side by side" {

    !identifiers hierarchical

    model {
        search = softwareSystem "Search" "Index and query" {
            idx = container "Indexer" "Builds index" "Java"
        }

        deploymentEnvironment "Dev" {
            deploymentNode "dev-box" {
                devInstance = containerInstance search.idx
            }
        }

        deploymentEnvironment "Prod" {
            deploymentNode "prod-cluster" {
                pod = deploymentNode "search-pod" "container host" "Kubernetes" {
                    prodInstance = containerInstance search.idx
                }
            }
        
    }

    views {
        deployment search "Dev" {
            include *
            autoLayout
        }

        deployment search "Prod" {
            include *
            autoLayout
        }
    }

}
