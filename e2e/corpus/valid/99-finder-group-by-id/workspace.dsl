workspace "Finder Group" "!elements over an identifier group" {

    properties {
        "structurizr.groupSeparator" "/"
    }

    model {
        department1 = group "Department 1" {
            a = softwareSystem "A" "First"
            b = softwareSystem "B" "Second"
        }

        !elements department1 {
            tag "Department-1"
        }
    }

}
