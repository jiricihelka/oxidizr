workspace "Dynamic Ordering" "Explicit numbering and overrides" {

    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        c = softwareSystem "C"

        a -> b "calls"
        b -> c "calls"
    }

    views {
        dynamic * "flow" "The order matters" {
            2: a -> b "step two"
            3: b -> c "step three"

            autoLayout
        }
    }

}
