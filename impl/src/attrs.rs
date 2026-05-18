use syn::parse::Parse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Attr {
    Immutable,
    Hide(Model),
    ServerOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Model {
    Delta,
    Draft,
    View,
}

impl Parse for Model {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        if ident == "delta" {
            Ok(Model::Delta)
        } else if ident == "draft" {
            Ok(Model::Draft)
        } else if ident == "view" {
            Ok(Model::View)
        } else {
            Err(syn::Error::new(ident.span(), "Unknown model"))
        }
    }
}

pub(crate) struct FieldAttrs {
    pub oxymorph: Vec<Attr>,
    pub other: Vec<syn::Attribute>,
}

pub(crate) fn parse_attrs(field: &syn::Field) -> Result<FieldAttrs, syn::Error> {
    let mut attrs = Vec::new();
    let mut other = Vec::new();
    for attr in &field.attrs {
        if attr.path().is_ident("oxymorph") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("immutable") {
                    attrs.push(Attr::Immutable);
                } else if meta.path.is_ident("hide") {
                    meta.parse_nested_meta(|model_meta| {
                        let model_path = &model_meta.path;
                        if model_path.is_ident("delta") {
                            attrs.push(Attr::Hide(Model::Delta));
                        } else if model_path.is_ident("draft") {
                            attrs.push(Attr::Hide(Model::Draft));
                        } else if model_path.is_ident("view") {
                            attrs.push(Attr::Hide(Model::View));
                        } else {
                            return Err(model_meta.error("unknown model"));
                        }
                        Ok(())
                    })?;
                } else if meta.path.is_ident("server_only") {
                    attrs.push(Attr::ServerOnly);
                } else {
                    return Err(meta.error("unknown attribute"));
                }
                Ok(())
            })?;
        } else {
            other.push(attr.clone());
        }
    }
    Ok(FieldAttrs { oxymorph: attrs, other })
}
