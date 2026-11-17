workspace "Flat Deployment Groups" "Groups beside the environment" {

    !identifiers hierarchical

    model {
        notify = softwareSystem "Notifier" "Push and email" {
            push = container "Push Gateway" "APNs/FCM" "Swift"
        }

        deploymentEnvironment "Prod" {
            blue = deploymentGroup "Blue"
            green = deploymentGroup "Green"

            deploymentNode "blue-1" {
                bi = containerInstance notify.push blue
            }
            deploymentNode "green-1" {
                gi = containerInstance notify.push green
            }
        }
    }

}
