workspace "Include In System" "Include from a system context" {

    model {
        shop = softwareSystem "Shop" "Storefront" {
            !include parts/containers.dsl
        }
        }
    }

}
