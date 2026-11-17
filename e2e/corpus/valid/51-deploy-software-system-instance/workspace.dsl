workspace "System Instance" "softwareSystemInstance on a node" {

    !identifiers hierarchical

    model {
        legacy = softwareSystem "Legacy Monolith" "The old thing"

        deploymentEnvironment "Coexistence" {
            deploymentNode "DC-1" {
                vm = deploymentNode "legacy-vm" "VM" "Hyper-V" "Windows" {
                    sInstance = softwareSystemInstance legacy
                }
            }
        }
    }

}
