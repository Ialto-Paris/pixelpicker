# Identity of Pixel Picker's MSIX package. Once the app's name is reserved in Partner Center,
# copy the three values from Product management > Product identity:
#   Name                  Package/Identity/Name
#   Publisher             Package/Identity/Publisher (CN=...)
#   PublisherDisplayName  Package/Properties/PublisherDisplayName
# Until then these placeholders build a package for local testing only: Partner Center rejects
# a package whose identity doesn't match the reserved app.
@{
    Name                 = 'Ialto.PixelPicker'
    Publisher            = 'CN=Ialto'
    PublisherDisplayName = 'Ialto'
}
