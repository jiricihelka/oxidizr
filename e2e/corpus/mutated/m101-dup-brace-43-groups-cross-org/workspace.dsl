workspace "Two Orgs" "Cross-organisation relationship" {

    model {
        group "Insurer" {
            underwriter = person "Underwriter" "Prices risk"
        }

        group "Broker" {
            agent = person "Agent" "Places business"
        }

        agent -> underwriter "submits slips to"
    }

}
}
