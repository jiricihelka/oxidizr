workspace "Unknown Var Pass" "Unresolved placeholders survive" {

    model {
        s = softwareSystem "${NOT_DEFINED_ANYWHERE}" "Stays raw"
    }

}
