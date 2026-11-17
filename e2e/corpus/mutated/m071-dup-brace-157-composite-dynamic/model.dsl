user = person "User"

portal = softwareSystem "Portal" "Public portal" {
    web = container "Web" "SPA host" "nginx" {
        formCtl = component "Form" "Collects input"
        validateCtl = component "Validate" "Checks input"
        submitCtl = component "Submit" "Sends off"

        formCtl -> validateCtl "submits to"
        validateCtl -> submitCtl "forwards valid to"
    }
    api = container "API" "Backend" "Go"

    web -> api "posts to"
}
