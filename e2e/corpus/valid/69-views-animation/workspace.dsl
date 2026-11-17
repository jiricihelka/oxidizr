workspace "View Animation" "Animation steps on a container view" {

    !identifiers hierarchical

    model {
        pipeline = softwareSystem "Pipeline" "CI" {
            runner = container "Runner" "Executes" "Go"
            store = container "Artifacts" "Blob store" "S3"

            runner -> store "uploads"
        }
    }

    views {
        container pipeline "containers" "Containers" {
            include *

            animation {
                pipeline.runner
                pipeline.store
            }
        }
    }

}
