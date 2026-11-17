workspace "Mixed Include Dirs" "Model and views from directories" {

    model {
        !include model

citizen -> ghostRelRefQz "applies via"
    }

    views {
        !include views
    }

}
