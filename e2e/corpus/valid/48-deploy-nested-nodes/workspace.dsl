workspace "Nested Deployment Nodes" "Cloud > subnet > host" {

    !identifiers hierarchical

    model {
        stock = softwareSystem "Stock Service" "Quotes prices" {
            api = container "Quote API" "HTTP surface" "Go"
        }

        deploymentEnvironment "Prod" {
            deploymentNode "Azure" "Cloud" "Region" {
                node = deploymentNode "quote-prod-01" "VM" "Virtual Machine" "Linux" {
                    apiInstance = containerInstance stock.api
                }
            }
        }
    }

    views {
        deployment stock "Prod" {
            include *
            autoLayout
        }
    }

}
