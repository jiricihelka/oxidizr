prod = deploymentEnvironment "Prod" {
    deploymentNode "gcp" "Cloud" "GCP" {
        deploymentNode "europe-west1" "Region" "Region" {
            cluster = deploymentNode "render-cluster" "GKE" "Kubernetes" {
                gwPool = deploymentNode "gw-pool" "Pool" "Node pool" {
                    gwI = containerInstance render.gateway
                }
                wkPool = deploymentNode "wk-pool" "Pool" "Node pool" {
                    w1 = containerInstance render.worker
                    w2 = containerInstance render.worker
                }
                qNode = deploymentNode "queue-1" "Memcached host" "VM" {
                    qI = containerInstance render.queue
                }
            }
        }
    }
}
