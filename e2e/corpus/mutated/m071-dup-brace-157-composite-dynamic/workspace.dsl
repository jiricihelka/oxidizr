workspace "Dynamic Flow Composite" "Component and dynamic views from files" {

    !identifiers hierarchical

    model {
        !include model.dsl

        user -> portal "signs up via"
    }

    views {
        !include views.dsl
    }

}
}
