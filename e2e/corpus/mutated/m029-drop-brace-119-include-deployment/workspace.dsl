workspace "Included Deployment" "Nodes from a file" {

    model {
        shop = softwareSystem "Shop" "Storefront" {
            web = container "Web" "UI" "Go"
            db = container "DB" "Store" "Postgres"
        }

        deploymentEnvironment "Live" {
            !include deploy/live.dsl
        
    }

}
