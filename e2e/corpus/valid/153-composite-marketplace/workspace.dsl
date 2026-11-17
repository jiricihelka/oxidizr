workspace "Marketplace" "Multi-seller marketplace" {

    !identifiers hierarchical

    model {
        !include model.dsl
        !include deployment.dsl
    }

    views {
        !include views.dsl

        styles {
            element "Internal" {
                background #2b8a3e
            }
        }
    }

}
