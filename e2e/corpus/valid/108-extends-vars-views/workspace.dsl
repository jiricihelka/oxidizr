workspace extends base.dsl {

    model {
        u = person "U" "A user"

        u -> ps "uses"
    }

    views {
        container ps "cv" "Containers of parent system" {
            include *
            autoLayout
        }
    }

}
