workspace "Flat Include Dir" "One directory of people files" {

    model {
        !include people

        portal = softwareSystem "Portal" "Self-service"

        clerk -> portal "serves via"
        broker -> portal "places via"
    }

}
