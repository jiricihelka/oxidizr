!var SYS "Claims"
!var TECH "Kotlin"

workspace "Vars Everywhere" "Substitution in all three positions" {

    model {
        ${SYS}Svc = softwareSystem "${SYS} System" "Handles ${SYS}" {
            engine = container "${SYS} Engine" "Core" "${TECH}"
            store = container "${SYS} Store" "Records" "Postgres"

            engine -> store "writes ${SYS} to"
        }
    }

}
