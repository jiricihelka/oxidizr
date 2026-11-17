workspace "Node Archetypes" "Deployment node archetypes" {

    model {
        archetypes {
            linuxHost = deploymentNode {
                technology "Linux"
            }
        }

        svc = softwareSystem "Svc" "Service" {
            core = container "Core" "Engine" "Go"
        }

        deploymentEnvironment "Live" {
            n = linuxHost "host-1" "Primary" "EC2" {
                ci = containerInstance core
            }
        
    }

}
