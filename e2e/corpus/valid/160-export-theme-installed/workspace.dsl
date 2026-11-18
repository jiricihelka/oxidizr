workspace "Installed Theme" "Theme loaded from the workspace themes/ dir" {

    model {
        user = person "User"
        api = softwareSystem "API" "Gateway" "Gateway"
        db = softwareSystem "Database" "Store" "Database"

        user -> api "calls"
        api -> db "reads"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        theme acme
    }

}
