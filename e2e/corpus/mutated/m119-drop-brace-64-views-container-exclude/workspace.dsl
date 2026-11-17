workspace "Containers Trimmed" "Container view excluding a container" {

    model {
        estate = softwareSystem "Estate" "Several containers" {
            app = container "App" "Main" "Java"
            admin = container "Admin" "Ops UI" "Python"
            db = container "DB" "Store" "Postgres"

            app -> db "reads"
            admin -> db "edits"
        }
    }

    views {
        container estate "containers" "Containers" {
            include *
            exclude admin

            autoLayout
        
    }

}
