systemLandscape "landscape" {
    include *
    autoLayout
}

component portal.web "web-components" "Inside the web container" {
    include *
    autoLayout
}

dynamic portal.web "signup-flow" "Signing up" {
    portal.web.formCtl -> portal.web.validateCtl "1. submit"
    portal.web.validateCtl -> portal.web.submitCtl "2. valid"

    autoLayout
}
