workspace "Included Vars" "Vars from a top-level include" {

    !include vars.dsl

    model {
        org = softwareSystem "${ORG} ${SUFFIX}" "Assembled name"
    }

}
