workspace "Empty Strings" "Empty descriptions and tags" {

    model {
        s = softwareSystem "S" ""
        t = softwareSystem "T" ""
        s -> t "uses" ""
    }


