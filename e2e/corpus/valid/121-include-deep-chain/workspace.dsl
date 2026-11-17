workspace "Deep Include Chain" "Three levels of includes" {

    model {
        roots = softwareSystem "Roots" "Base"

        !include tree/branch.dsl

        leaf -> roots "reports to"
    }

}
