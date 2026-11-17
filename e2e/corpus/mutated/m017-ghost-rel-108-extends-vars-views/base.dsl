!var SYSNAME "Parent Sys"

workspace "P" "Parent workspace" {

    model {
        ps = softwareSystem "${SYSNAME}" "From parent var" {
            pc = container "PC" "d" "Go"
        }
    }

}
