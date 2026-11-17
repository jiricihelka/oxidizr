workspace "Tagged Links" "Relationships with tags" {

    model {
        batch = softwareSystem "Nightly Batch" "End-of-day jobs"
        ledger = softwareSystem "Ledger" "Books entries"
        report = softwareSystem "Regulatory Reporting" "Files returns"

        batch -> ledger "posts journal entries" "Batch,Financial"
        batch -> report "feeds cubed data" "Batch,Regulatory"
    }

    views {
        systemLandscape "landscape" {
            include *

            autoLayout
        }
    }

}
