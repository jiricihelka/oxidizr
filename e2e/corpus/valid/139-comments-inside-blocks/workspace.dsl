workspace "Comments Inside" "Comments between statements in contexts" {

    model {
        s = softwareSystem "S" {
            // before the property block
            properties {
                // inside the property block
                "k" "v"
            }
            // before the container
            c = container "C" "d" "T"
        }
    }

}
