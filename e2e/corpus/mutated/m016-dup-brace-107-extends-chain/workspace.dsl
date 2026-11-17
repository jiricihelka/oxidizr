workspace extends parent.dsl {

    model {
        edge = softwareSystem "Edge" "The edge"

        edge -> mid "calls"
    }

}
}
