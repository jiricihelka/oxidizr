workspace "This And That" "Relationships built with the this keyword" {

    model {
        auth = softwareSystem "Auth Service" "Issues tokens" {
            issuer = container "Token Issuer" "OIDC provider" "Keycloak" {
                policy = component "Policy Engine" "Evaluates claims"
                audit = component "Audit Sink" "Records token grants"

                policy -> audit "emits grants to"
            }
            cache = container "Session Cache" "Hot sessions" "Redis"

            issuer -> cache "caches sessions in"
            cache -> issuer "revalidates against"
        }
    }

    views {
        container auth "containers" {
            include *
            autoLayout
        }
    }

}
