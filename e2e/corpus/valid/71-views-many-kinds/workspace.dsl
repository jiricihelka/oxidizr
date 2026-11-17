workspace "Many View Kinds" "Landscape context container dynamic together" {

    !identifiers hierarchical

    model {
        client = person "Client"
        core = softwareSystem "Core" "The core" {
            api = container "API" "REST" "Java" {
                users = component "Users" "User mgmt"
                auth = component "Auth" "Login"

                auth -> users "loads"
            }
        }

        client -> core.api "uses"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        systemContext core "core-context" {
            include *
            autoLayout
        }

        container core "core-containers" {
            include *
            autoLayout
        }

        component core.api "core-components" {
            include *
            autoLayout
        }

        dynamic core.api "login-flow" {
            core.api.auth -> core.api.users "1. authenticate"

            autoLayout
        }
    }

}
