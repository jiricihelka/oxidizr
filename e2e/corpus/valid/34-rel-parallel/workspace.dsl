workspace "Parallel Lanes" "Two relationships between the same pair" {

    model {
        website = softwareSystem "Company Website" "Marketing site"
        search = softwareSystem "Search Service" "Full-text index"

        website -> search "sends indexing updates to" "gRPC"
        website -> search "answers autocomplete via" "HTTPS"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
