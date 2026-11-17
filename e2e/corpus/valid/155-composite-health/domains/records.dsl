records = softwareSystem "Records" "EHR" {
    ehrApi = container "EHR API" "FHIR facade" "Java"
    ehrDb = container "EHR Store" "Clinical" "Postgres"

    ehrApi -> ehrDb "reads and writes"
}
