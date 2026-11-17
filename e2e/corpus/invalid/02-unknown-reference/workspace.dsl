workspace "Dangling" "Relationship to an undeclared element" {

    model {
        a = softwareSystem "Alpha"

        a -> ghost "talks to"
    }

}
