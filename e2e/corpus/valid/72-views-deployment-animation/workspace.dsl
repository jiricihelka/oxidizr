workspace "Deployment Animation" "Animation over instances" {

    model {
        quotes = softwareSystem "Quotes" "Pricing" {
            svc = container "Quote API" "REST" "Go"
        }

        deploymentEnvironment "Live" {
            deploymentNode "node-1" {
                q1 = containerInstance svc
            }
        }
    }

    views {
        deployment quotes "Live" {
            include *

            animation {
                q1
            }
        }
    }

}
