workspace "Finder Parent" "!elements by parent" {

    model {
        u = person "U"

        app = softwareSystem "App" "Web app" {
            c1 = container "Web" "UI" "Go"
            c2 = container "API" "Service" "Go"
            db = container "DB" "Store" "SQLite"

            c1 -> db "queries"
        }

        !elements "element.parent==app && element.technology==Go" {
            tag "GoLayer"
            u -> this "reaches"
        }
    }

}
