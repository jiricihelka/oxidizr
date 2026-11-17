workspace "Finder Nested Groups" "Finders over nested groups" {

    model {
        properties {
            "structurizr.groupSeparator" "/"
        }

        department1 = group "Department 1" {
            team1 = group "Team 1" {
                s1 = softwareSystem "S1" "Alpha"
            }
            team2 = group "Team 2" {
                s2 = softwareSystem "S2" "Beta"
            }
        }

        !elements team1 {
            tag "Team-1"
        }

        !elements department1 {
            url "https://wiki.example.com/dept1"
        }
    }

}
