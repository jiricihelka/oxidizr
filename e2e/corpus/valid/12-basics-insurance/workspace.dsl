workspace "Claims Desk" "Motor insurance claims handling" {

    model {
        claimant = person "Claimant" "Files a motor claim"
        adjuster = person "Claims Adjuster" "Assesses damage reports"

        portal = softwareSystem "Claims Portal" "Self-service claim intake"
        assessment = softwareSystem "Assessment Tools" "Estimates repair costs"

        claimant -> portal "submits claims via"
        adjuster -> assessment "prices repairs in"
        portal -> assessment "forwards claims to"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        systemContext portal "portal-context" {
            include *
            autoLayout
        }
    }

}
