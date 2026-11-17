workspace extends base.dsl {

    model {
        !element live {
            deploymentNode "app-1" {
                apiI = containerInstance svc.api
            }
        }
    }

    views {
        deployment svc "Live" "live-deployment" "Live deployment" {
            include *
            autoLayout
        }
    }

}
