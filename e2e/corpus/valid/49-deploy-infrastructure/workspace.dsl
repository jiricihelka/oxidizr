workspace "Infrastructure Nodes" "LB and queue beside hosts" {

    !identifiers hierarchical

    model {
        shop = softwareSystem "Shop" "Storefront" {
            web = container "Web" "UI" "Java"
        }

        deploymentEnvironment "Live" {
            deploymentNode "AWS" {
                lb = infrastructureNode "Load Balancer" "Public entry" "ALB"
                host = deploymentNode "web-1" "EC2" "EC2" "Amazon Linux" {
                    webInstance = containerInstance shop.web
                }
                broker = infrastructureNode "Broker" "Message broker" "Amazon MQ"
            }
        }
    }

    views {
        deployment shop "Live" {
            include *
            autoLayout
        }
    }

}
