workspace "Boxes And Systems" "Landscape boxes around systems" {

    model {
        channel = element "Channels" "Domain" "Customer touchpoints" "Domain"
        core = element "Core" "Domain" "Policy administration" "Domain"

        web = softwareSystem "Web Branch" "Self-service"
        coreSys = softwareSystem "Policy Admin" "Policy lifecycle"

web -> ghostRelRefQz "submits quotes to"
    }

    views {
        custom "landscape" "Domain Landscape" "Boxes and systems" {
            include *
            autolayout
        }
    }

}
