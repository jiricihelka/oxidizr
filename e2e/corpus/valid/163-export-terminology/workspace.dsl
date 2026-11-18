workspace "Terminology Export" "Renamed element kinds flow into diagram types" {

    model {
        user = person "User"
        front = softwareSystem "Frontend" "Web UI"
        store = softwareSystem "Store" "Backend" {
            core = container "Core" "Core service" "Go"
            data = container "Data" "Storage" "PostgreSQL"
            cache = container "Cache" "Edge cache" "Varnish" {
                technology "Cache"
            }

            core -> data "persists to"
            core -> cache "reads through"
        }

        user -> front "browses"
        front -> core "requests"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        container store "containers" {
            include *
            autoLayout
        }

        terminology {
            person "Actor"
            softwareSystem "Service"
            container "Subsystem"
            component "Module"
            deploymentNode "Server"
            infrastructureNode "Runtime"
            relationship "Depends on"
        }

        styles {
            element "Cache" {
                background "#FFF3CD"
                stroke "#8A6D0B"
                strokeWidth 1
                color "#5C4A06"
            }

            relationship "Depends on" {
                dashed true
                color "#8A8A8A"
                thickness 1
            }
        }
    }

}
