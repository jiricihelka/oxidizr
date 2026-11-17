!var ORG "Globex"
!var REGION "eu-west-1"

workspace "Vars Basic" "Variable substitution" {

    model {
        ops = person "Ops ${ORG}" "Staff in ${REGION}"

        fleet = softwareSystem "${ORG} Fleet" "Vehicles" {
            telemetry = container "Telemetry ${REGION}" "Ingest" "Go"
        }

        ops -> fleet "drives"
    }

}
