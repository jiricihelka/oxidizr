!var NAME "First"
!var NAME "Second"

workspace "Vars Redefined" "Last definition wins" {

    model {
        s = softwareSystem "${NAME}" "Should be Second"
    }

}
