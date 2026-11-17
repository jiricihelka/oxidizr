workspace "Deployment Groups" "Container instances grouped per instance" {

    !identifiers hierarchical

    model {
        calc = softwareSystem "Calc" "Maths" {
            engine = container "Engine" "Compute" "Rust"
        }

        deploymentEnvironment "Prod" {
            group1 = deploymentGroup "Instance 1"
            group2 = deploymentGroup "Instance 2"

            deploymentNode "node-1" {
                i1 = containerInstance calc.engine group1
            }
            deploymentNode "node-2" {
                i2 = containerInstance calc.engine group2
            }
        }
    }

    views {
        deployment calc "Prod" {
            include *
            autoLayout
        }
    }

}
