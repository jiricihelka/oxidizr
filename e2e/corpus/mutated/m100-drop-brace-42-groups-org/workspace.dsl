workspace "Organised Groups" "Organisational grouping via groups" {

    model {
        group "Acme Group" {
            clerk = person "Clerk" "Branch staff"
            manager = person "Branch Manager" "Approves limits"

            clerk -> manager "escalates to"
        }
    

}
