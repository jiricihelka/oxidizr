workspace "Dep Base" "Model and env" {

    !identifiers hierarchical

    model {
        svc = softwareSystem "Service" "Does work" {
            api = container "API" "REST" "Go"
        }

        live = deploymentEnvironment "Live" {
            deploymentNode "edge"
        }
    }

}
