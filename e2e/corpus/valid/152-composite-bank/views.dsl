systemLandscape "landscape" "Banking landscape" {
    include *
    autoLayout
}

systemContext onlineBanking "banking-context" "Online banking context" {
    include *
    autoLayout
}

container onlineBanking "containers" "Inside online banking" {
    include *
    autoLayout
}

component onlineBanking.backend "backend-components" "Backend internals" {
    include *
    autoLayout
}
