# a hash comment at top level
// a slash comment at top level
/* a block comment
   across lines */
workspace "Comments" "All comment kinds" {

    // inside workspace
    # also inside
    model {
        /* block inside model */
        s = softwareSystem "S"
        # leading comment
        t = softwareSystem "T"
        s -> t "calls"
    }

}
