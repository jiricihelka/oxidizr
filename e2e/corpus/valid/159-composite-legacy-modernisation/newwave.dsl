strangler = softwareSystem "Strangler" "Facade over the monolith" {
    facade = container "Facade" "Routes calls" "Go"
    newSvc = container "New Service" "Rewritten domain" "Go"
    kafka = container "Kafka" "Dual-write bus" "Kafka"

    facade -> newSvc "routes new"
    facade -> legacyCore.monolith "falls back to"
    newSvc -> kafka "emits"
}
