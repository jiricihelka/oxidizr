workspace "Punctuation, Eh?" "Names and descriptions with commas, braces {like these} and colons: semicolons;" {

    model {
        ops = person "Ops — on-call" "Receives pages (24x7); triages; escalates"

        alerting = softwareSystem "Alerting Hub" "Routes alerts: Slack, e-mail, phone" {
            properties {
                "oncall.rotation" "weekly"
            }
        }

        ops -> alerting "acknowledges alerts from"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
