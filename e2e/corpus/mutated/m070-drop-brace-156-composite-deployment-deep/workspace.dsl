workspace "Deep Deployment Composite" "Four-level node tree from files" {

    !identifiers hierarchical

    model {
        !include parts/model.dsl
        !include parts/deploy.dsl
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        deployment render "Prod" "prod-view" "Prod topology" {
            include *

            animation {
                render.gateway
                render.queue
                render.worker
            }
        }
    

}
