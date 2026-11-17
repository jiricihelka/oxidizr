workspace "Finder Tags Then Link" "Finder adds tags then links them" {

    model {
        monitor = softwareSystem "Monitor" "Observability"

        estate = softwareSystem "Estate" "Containers" {
            a = container "A" "First" "Go"
            b = container "B" "Second" "Go"
            c = container "C" "Third" "Java"
        }

        !elements "element.technology==Go" {
            tags "Watched"
            monitor -> this "scrapes"
        }
    }

}
