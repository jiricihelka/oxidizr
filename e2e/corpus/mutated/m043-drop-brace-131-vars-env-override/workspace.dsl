!var STAGE "dev"

workspace "Vars Env Slot" "Environment name composed" {

    model {
        app = softwareSystem "App" "Runs on ${STAGE}" {
            api = container "API ${STAGE}" "Service" "Go"
        

        deploymentEnvironment "On ${STAGE}" {
            deploymentNode "app-${STAGE}" "Host" "EC2" "Linux"
        }
    }

}
