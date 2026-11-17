workspace "Group Archetypes" "Group archetype sets the separator" {

    model {
        archetypes {
            microservice = group
        }

        x = softwareSystem "X" "System" {
            customerService = microservice "Customer Service" {
                db = container "Customer DB" "Store" "Postgres"
                api = container "Customer API" "REST" "Go"

                api -> db "reads"
            }
        
    }

}
