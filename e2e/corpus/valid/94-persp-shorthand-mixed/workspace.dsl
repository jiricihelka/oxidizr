workspace "Perspective Shorthand Mix" "Two and three argument perspectives" {

    model {
        legacy = softwareSystem "Legacy Core" "The old one" {
            perspectives {
                "Risk" "Single point of failure" "Critical"
                "Cost" "Expensive mainframe time"
                "Roadmap" "Strangle out by 2028" "Medium"
            }
        }
    }

}
