!var BASE_URL "https://example.com"

workspace "Vars Build URLs" "Composing URLs from vars" {

    model {
        portal = softwareSystem "Portal" "Customer portal" {
            url "${BASE_URL}/portal"
            ui = container "Portal UI" "SPA" "React" {
                url "${BASE_URL}/ui"
            
        }
    }

}
