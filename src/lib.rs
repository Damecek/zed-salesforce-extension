use zed_extension_api as zed;

struct ApexExtension;

impl zed::Extension for ApexExtension {
    fn new() -> Self {
        Self
    }
}

zed::register_extension!(ApexExtension);
