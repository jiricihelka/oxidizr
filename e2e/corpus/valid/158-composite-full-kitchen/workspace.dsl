!include lib/consts.dsl

workspace "${SYSTEM} Estate" "Kitchen-sink composite" {

    !identifiers hierarchical

    properties {
        "structurizr.groupSeparator" "${GROUP_SEP}"
    }

    model {
        !include model.dsl
    }

    views {
        systemLandscape "landscape" "Estate" {
            title "${SYSTEM} Landscape"
            include *
            autoLayout
        }

        container platform "platform-containers" "Inside the platform" {
            include *
            autoLayout
        }

        component platform.routing "routing-components" "Inside routing" {
            include *
            autoLayout
        }

        styles {
            element "Service" {
                background #5f3dc4
                color #ffffff
            }
        }

        theme default
    }

}
