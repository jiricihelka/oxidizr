workspace extends child-a.dsl {

    model {
        b = softwareSystem "B" "Sibling B"

        b -> core "uses"
a -> ghostRelRefQz "talks to"
    }

}
