workspace "Instances Count" "Replicas via instances" {

    !identifiers hierarchical

    model {
        render = softwareSystem "Renderer" "Charts server" {
            worker = container "Render Worker" "Headless browser" "Go"
        }

        deploymentEnvironment "Prod" {
            deploymentNode "k8s" "Cluster" "K8s" "Kubernetes" {
                pool = deploymentNode "render-pool" "Node pool" "Node pool" "Linux" {
                    w1 = containerInstance render.worker
                    w2 = containerInstance render.worker
                    w3 = containerInstance render.worker
                }
            }
        }
    }

    views {
        deployment render "Prod" {
            include *
            autoLayout
        
    }

}
