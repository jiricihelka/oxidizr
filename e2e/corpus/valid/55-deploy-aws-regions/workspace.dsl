workspace "Multi Region AWS" "Two regions one app" {

    !identifiers hierarchical

    model {
        feed = softwareSystem "Feed" "Content feed" {
            svc = container "Feed Service" "Serves feed" "Node.js"
            cache = container "Cache" "Hot items" "Redis"
        }

        deploymentEnvironment "Global" {
            deploymentNode "AWS" "Cloud" "Global" "Amazon Web Services - Regions" {
                eu = deploymentNode "eu-west-1" "Region" "Region" "Amazon EC2" {
                    euHost = deploymentNode "feed-eu-1" "Host" "EC2" "Linux" {
                        euSvc = containerInstance feed.svc
                        euCache = containerInstance feed.cache
                    }
                }
                us = deploymentNode "us-east-1" "Region" "Region" "Amazon EC2" {
                    usHost = deploymentNode "feed-us-1" "Host" "EC2" "Linux" {
                        usSvc = containerInstance feed.svc
                    }
                }
            }
        }
    }

    views {
        deployment feed "Global" {
            include *
            autoLayout
        }
    }

}
