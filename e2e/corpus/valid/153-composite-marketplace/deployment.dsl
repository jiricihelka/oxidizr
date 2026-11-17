live = deploymentEnvironment "Live" {
    deploymentNode "AWS" "Cloud" "Global" {
        deploymentNode "eu-west-1" "Region" "Region" {
            hostA = deploymentNode "market-a" "EC2" "EC2" "Amazon Linux" {
                storefrontI = containerInstance marketplace.storefront
                ordersI = containerInstance marketplace.orders
            }
            hostB = deploymentNode "market-b" "EC2" "EC2" "Amazon Linux" {
                ledgerI = containerInstance marketplace.ledger
            }
            dbNode = deploymentNode "market-db" "RDS" "RDS" "PostgreSQL" {
                dbI = containerInstance marketplace.postgres
            }
        }
    }
}
