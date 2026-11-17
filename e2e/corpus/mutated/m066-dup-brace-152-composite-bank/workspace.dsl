!include constants.dsl

workspace "${BANK}" "Enterprise banking estate" {

    !identifiers hierarchical

    model {
        !include people.dsl
        !include systems.dsl
        !include relationships.dsl
    }
    }

    views {
        !include views.dsl
        !include styles.dsl
    }

}
