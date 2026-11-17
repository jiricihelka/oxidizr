workspace "Mixed Include Dirs" "Model and views from directories" {

    model {
        !include model

        citizen -> permits "applies via"
    }

    views {
        !include views
    }

}
