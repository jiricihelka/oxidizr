workspace "Model Only" "A model with no views at all" {

    model {
        bot = softwareSystem "Crawler" "Indexes partner catalogs"
        api = softwareSystem "Catalog API" "Serves product data"

        bot -> api "crawls"
    }

}
