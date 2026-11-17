workspace "Relationship Blocks" "Relationship context with url and properties" {

    model {
        scm = softwareSystem "Source Control" "Git hosting"
        ci = softwareSystem "Build Service" "CI runners"

        scm -> ci "triggers builds" "Webhook" {
            url "https://hooks.example.com/ci"
            properties {
                "retry.policy" "exponential"
                "max.inflight" "8"
            }
        
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
