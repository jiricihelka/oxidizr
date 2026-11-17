workspace "Continued" "Statements split with backslashes" {

    model {
        s = \
            softwareSystem \
            "Split Name" \
            "Split description"

        t = softwareSystem "T"

        s \
            -> t \
            "split link"
    }

}
