workspace "Deployed Service" "Deployment environment and view" {

    !identifiers hierarchical

    model {
        ops = person "Operator"

        billing = softwareSystem "Billing" "Charges customers" {
            api = container "Billing API" "API" "Go"
            db = container "Billing DB" "Storage" "PostgreSQL"
            api -> db "stores"
        }

        ops -> billing.api "operates"

        deploymentEnvironment "Live" {
            deploymentNode "AWS" "AWS cloud" "Region" {
                internet = deploymentNode "Internet"

                vpc = deploymentNode "VPC" "Virtual private cloud" "VPC" {
                    privateSubnet = deploymentNode "Private" "Private subnet" "Subnet" {
                        host = deploymentNode "billing-host" "EC2 host" "Amazon EC2" "Linux" {
                            apiInstance = containerInstance billing.api
                        }
                    }

                    dataSubnet = deploymentNode "Data" "Data subnet" "Subnet" {
                        dbHost = deploymentNode "billing-db" "RDS host" "Amazon RDS" "Linux" {
                            dbInstance = containerInstance billing.db
                        }
                    }
                }
            }
        }
    }

    views {
        deployment billing "Live" {
            include *
            autoLayout
        }
    }

}
