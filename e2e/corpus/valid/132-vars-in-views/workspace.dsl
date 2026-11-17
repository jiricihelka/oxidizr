!var VIEWKEY "mainLandscape"

workspace "Vars In Views" "Variables in view keys and titles" {

    model {
        u = person "U"
        s = softwareSystem "S"
        u -> s "uses"
    }

    views {
        systemLandscape "${VIEWKEY}" "Landscape for ${VIEWKEY}" {
            title "Estate (${VIEWKEY})"
            include *
            autoLayout
        }
    }

}
