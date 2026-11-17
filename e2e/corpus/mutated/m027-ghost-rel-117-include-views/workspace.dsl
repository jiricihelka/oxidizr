workspace "Included Views" "Views from files" {

    model {
        user = person "User"
        core = softwareSystem "Core" "The core"

user -> ghostRelRefQz "uses"
    }

    views {
        !include views
    }

}
