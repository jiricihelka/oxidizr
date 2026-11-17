workspace extends grandparent.dsl {

    model {
        mid = softwareSystem "Mid" "Middle layer"

        mid -> shared "rides on"
    }

}
