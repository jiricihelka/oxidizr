workspace "Finder All Relationships" "Bulk tag every link" {

    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        c = softwareSystem "C"

        a -> b "calls"
        b -> c "calls"

        !relationships "*->*" {
            tag "Audited"
        }
    }

}
