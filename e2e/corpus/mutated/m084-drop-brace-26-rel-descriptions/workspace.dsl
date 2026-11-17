workspace "Described Links" "Relationships with varying descriptions" {

    model {
        author = person "Author" "Writes content"
        cms = softwareSystem "CMS" "Authoring and publishing"
        cdn = softwareSystem "CDN" "Edge cache"

        author -> cms "drafts articles in"
        cms -> cdn "purges after publish"
        cdn -> cms "fetches origin content from"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    

}
