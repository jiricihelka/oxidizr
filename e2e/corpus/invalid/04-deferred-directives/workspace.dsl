workspace "Deferred Directives" "Directives whose importers land in later phases" {

    !docs docs/exported.md some.importer.Importer

    !impliedRelationships com.example.NoStrategy

    model {
        s = softwareSystem "System"
    }

    views {
        systemLandscape "l" {
            include *
            autoLayout
        }
    }

}
