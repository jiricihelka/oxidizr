workspace "Container Archetypes" "Container archetypes with tags and technology" {

    model {
        archetypes {
            service = container {
                technology "Go"
                tags "Service"
            }
            datastore = container {
                technology "Postgres"
                tags "Datastore"
            }
        }

        shop = softwareSystem "Shop" "Storefront" {
            api = service "Shop API" "REST surface"
            db = datastore "Shop DB" "Orders"

api -> ghostRelRefQz "persists to"
        }
    }

}
