/*
    A block comment that mentions "quotes" and // slashes and ends here.
*/
workspace "Stress Mix" "Quotes \"inside\" descriptions beside # comments" {

    model {
        s = softwareSystem "A \"quoted\" system // with slashes" "Hash # in text"
        // a comment after a quoted system
        t = softwareSystem "T"

        s -> t "\"quoted\" link description"
    }
    }

}
