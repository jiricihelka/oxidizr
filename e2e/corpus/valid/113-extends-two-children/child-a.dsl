workspace extends base.dsl {

    model {
        a = softwareSystem "A" "Child A"

        a -> core "uses"
    }

}
