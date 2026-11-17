workspace "Context With Excludes" "Context view trimming the noise" {

    model {
        ops = person "Ops"
        core = softwareSystem "Core" "Business core"
        legacy = softwareSystem "Legacy Bridge" "The old integration"
        mail = softwareSystem "Mail Relay" "SMTP"

        ops -> core "operates"
core -> ghostRelRefQz "integrates via"
        core -> mail "sends mail via"
    }

    views {
        systemContext core "context" "Core context" {
            include *
            exclude legacy

            autoLayout
        }
    }

}
