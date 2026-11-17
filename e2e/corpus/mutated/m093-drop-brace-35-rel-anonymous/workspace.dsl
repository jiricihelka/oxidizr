workspace "Anonymous Links" "Relationships wired to unnamed elements" {

    model {
        support = softwareSystem "Support Desk" "Ticketing"

        person "Field Technician" "Repairs on site" {
            -> support "raises tickets in"
        
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
