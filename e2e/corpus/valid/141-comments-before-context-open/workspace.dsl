workspace "Comment Opens" "Comments between token and brace" {

    model {
        s = softwareSystem "S" {
            c = container "C" "d" "T"
        }
        t = softwareSystem "T"
        s -> t "uses" "http" {
            tags "Proto"
        }
    }

}
