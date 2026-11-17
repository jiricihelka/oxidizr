workspace "Style Groups" "Styling the Group tag" {

    properties {
        "structurizr.groupSeparator" "/"
    }

    model {
        group "Payslips" {
            hr = softwareSystem "HR" "People"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        styles {
            element "Group" {
                shape RoundedBox
                background #f1f3f5
                stroke #adb5bd
                fontSize 12
            }
        }
    }

}
