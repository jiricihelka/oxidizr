workspace "Health Checks" "Checks on instances" {

    !identifiers hierarchical

    model {
        api = softwareSystem "API" "Public API" {
            gw = container "Gateway" "Edge" "Envoy"
        }

        deploymentEnvironment "Prod" {
            deploymentNode "edge-1" {
                gwInstance = containerInstance api.gw {
                    healthCheck "liveness" "https://edge-1/healthz"
                    healthCheck "readiness" "https://edge-1/readyz" 60
                    healthCheck "deep" "https://edge-1/deep" 120 1000
                }
            }
        }
    }

}
