!const ENVIRONMENT "prod"
!const REPLICA_COUNT 3

workspace "Constants Basic" "Constant substitution" {

    model {
        api = softwareSystem "API" "Served in ${ENVIRONMENT}" {
            gw = container "Gateway ${ENVIRONMENT}" "Edge" "Envoy"
        }
    }

}
}
