workspace "Deep Node Tree" "Four levels of nodes" {

    !identifiers hierarchical

    model {
        batch = softwareSystem "Batch" "Nightly jobs" {
            runner = container "Job Runner" "Executes jobs" "Python"
        }

        deploymentEnvironment "Prod" {
            cloud = deploymentNode "GCP" "Cloud" "GCP" {
                region = deploymentNode "europe-west1" "Region" "Region" {
                    cluster = deploymentNode "batch-cluster" "GKE" "Kubernetes" {
                        pool = deploymentNode "nightly-pool" "Pool" "Node pool" {
                            jobRunner = containerInstance batch.runner
                        }
                    }
                }
                }
            }
        }
    }

    views {
        deployment batch "Prod" {
            include *
            autoLayout
        }
    }

}
