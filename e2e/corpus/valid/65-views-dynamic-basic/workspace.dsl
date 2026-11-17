workspace "Dynamic Basic" "A dynamic view over a container" {

    !identifiers hierarchical

    model {
        signup = softwareSystem "Signup" "Registration" {
            api = container "Signup API" "Endpoints" "Go" {
                svc = component "Signer" "Writes agreements"
                mailer = component "Mailer" "Sends welcome mail"

                svc -> mailer "triggers"
            }
        }
    }

    views {
        dynamic signup.api "signup-flow" "Signing up" {
            signup.api.svc -> signup.api.mailer "1. user signs"
            signup.api.mailer -> signup.api.svc "2. queued"

            autoLayout
        }
    }

}
