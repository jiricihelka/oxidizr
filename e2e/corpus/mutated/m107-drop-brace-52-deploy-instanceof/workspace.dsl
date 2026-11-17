workspace "Instance Of" "instanceOf shorthand for nodes" {

    !identifiers hierarchical

    model {
        portal = softwareSystem "Portal" "Public portal" {
            front = container "Frontend" "SPA host" "nginx"
        }

        deploymentEnvironment "Prod" {
            deploymentNode "Cloud" {
                n1 = instanceOf portal.front
            }
        }
    }


