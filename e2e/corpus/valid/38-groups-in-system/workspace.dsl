workspace "Grouped Containers" "Container groups inside a system" {

    model {
        platform = softwareSystem "Internal Platform" "Paved road" {
            group "Runtime" {
                k8s = container "Kubernetes" "Workload host" "K8s"
                mesh = container "Service Mesh" "mTLS and routing" "Istio"
            }

            group "Observability" {
                metrics = container "Metrics" "TSDB" "Prometheus"
            }

            mesh -> k8s "routes into"
            metrics -> k8s "scrapes"
        }
    }

    views {
        container platform "containers" {
            include *
            autoLayout
        }
    }

}
