workspace "Nested Include Dir" "Recursive directory include" {

    model {
        !include model

        reporting -> core "reads from"
    }


