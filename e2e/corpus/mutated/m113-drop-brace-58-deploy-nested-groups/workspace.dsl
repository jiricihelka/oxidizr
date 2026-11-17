workspace "Nested Deployment Groups" "Groups of groups" {

    !identifiers hierarchical

    model {
        cart = softwareSystem "Cart" "Basket" {
            api = container "Cart API" "REST" "Kotlin"
        }

        deploymentEnvironment "Prod" {
            cells = deploymentGroup "Cells"
            cell1 = deploymentGroup "Cell 1"
            cell2 = deploymentGroup "Cell 2"

            deploymentNode "cell1-host" {
                c1 = containerInstance cart.api cell1
            
            deploymentNode "cell2-host" {
                c2 = containerInstance cart.api cell2
            }
        }
    }

}
