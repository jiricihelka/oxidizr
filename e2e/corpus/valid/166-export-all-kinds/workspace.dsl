workspace "All Kinds Export" "Every view kind in one clean workspace — the biggest export surface" {

    !identifiers hierarchical

    model {
        user = person "Ops"
        customer = softwareSystem "Customer" "Upstream"

        sales = element "Front office" "Domain" "Sales" "SalesDomain"
        fulfil = element "Back office" "Domain" "Fulfilment" "SalesDomain"

        sales -> fulfil "handoff"

        stock = softwareSystem "Stock" "Inventory core" {
            api = container "Stock API" "Endpoints" "Java" {
                svc = component "StockService" "Business rules"
                repo = component "StockRepo" "JDBC access"

                svc -> repo "queries"
            }
            ui = container "Console" "Admin UI" "TypeScript"

            user -> api "manages via"
            ui -> api "drives"
        }

        db = softwareSystem "Postgres" "Database" {
            main = container "Primary" "Primary instance" "PostgreSQL"
        }
        stock.api -> db "stores in"
        customer -> stock.api "reserves at"

        deploymentEnvironment "Live" {
            amazon = deploymentNode "AWS" "Cloud" "eu-west-1" {
                vpc = deploymentNode "vpc-prod" "VPC" "Network" {
                    host = deploymentNode "app-01" "VM" "Linux" {
                        apiInstance = containerInstance stock.api
                    }
                    dbhost = deploymentNode "db-01" "RDS" "Managed" {
                        dbInstance = containerInstance db.main
                    }
                }
            }
            edge = deploymentNode "Edge" "On-prem" "Rack 4" {
                consoleBox = containerInstance stock.ui
            }
        }

        deploymentEnvironment "Staging" {
            stage = deploymentNode "staging-01" "VM" "Linux" {
                apiStage = containerInstance stock.api
            }
        }
    }

    views {
        systemLandscape "landscape" {
            include *
        }

        systemContext stock "context" {
            include *
            autoLayout
        }

        container stock "containers" {
            include *
            autoLayout
        }

        component stock.api "components" {
            include *
            autoLayout
        }

        dynamic stock.api "flow" "A reservation lands" {
            stock.api.svc -> stock.api.repo "1. check stock"
            stock.api.repo -> stock.api.svc "2. rows"

            autoLayout
        }

        deployment stock "Live" "prod-view" "Production map" {
            include *
            autoLayout
        }

        filtered landscape exclude Database "no-db" "Storage hidden"

        custom "custom-domains" "Domains" "Boxes side by side" {
            include sales fulfil
            autolayout
        }

        styles {
            element "Container" {
                background "#438DD5"
                color "#ffffff"
            }

            element "Database" {
                shape Cylinder
            }
        }
    }

}
