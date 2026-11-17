workspace "Finder Element Extend" "Add children to an existing element" {

    model {
        sys = softwareSystem "S" "Existing"

        !element sys {
            api = container "API" "Surface" "Go"
            db = container "DB" "Store" "Postgres"

            api -> db "reads"
        }
    }

}
