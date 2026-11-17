legacyCore = softwareSystem "Legacy Core" "The monolith" {
    monolith = container "Monolith" "Everything" "Java 6"
    coreDb = container "Core DB" "Data" "Oracle"

    monolith -> coreDb "reads"
}
