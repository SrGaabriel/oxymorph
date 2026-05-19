use syn::{
    Token,
    parse::{Parse, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
};

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
    Ok(FieldAttrs {
        oxymorph: attrs,
        other,
    })
}

pub struct ModelArgs {
    pub variants: Vec<Model>,
    pub sea_orm: Option<SeaOrmArgs>,
}

pub struct SeaOrmArgs {
    pub entity: syn::Path,
}

impl Parse for ModelArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "expected at least one of: delta, draft, view",
            ));
        }
        let mut variants = Vec::new();
        let mut sea_orm = None;
        let punctuated = Punctuated::<syn::Meta, Token![,]>::parse_terminated(input)?;
        for meta in punctuated {
            if meta.path().is_ident("delta") {
                variants.push(Model::Delta);
            } else if meta.path().is_ident("draft") {
                variants.push(Model::Draft);
            } else if meta.path().is_ident("view") {
                variants.push(Model::View);
            } else if meta.path().is_ident("sea_orm") {
                if let syn::Meta::List(list) = meta {
                    sea_orm = Some(SeaOrmArgs::parse.parse2(list.tokens)?);
                } else {
                    return Err(syn::Error::new(meta.span(), "expected sea_orm(...)"));
                }
            } else {
                return Err(syn::Error::new(meta.span(), "unknown model argument"));
            }
        }
        if variants.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "expected at least one of: delta, draft, view",
            ));
        }
        Ok(ModelArgs { variants, sea_orm })
    }
}

impl Parse for SeaOrmArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let punctuated = Punctuated::<syn::Meta, Token![,]>::parse_terminated(input)?;
        let mut entity = None;
        for meta in punctuated {
            if meta.path().is_ident("entity") {
                if let syn::Meta::NameValue(nv) = meta {
                    let expr: &syn::Expr = &nv.value;
                    if let syn::Expr::Path(expr_path) = expr {
                        entity = Some(expr_path.path.clone());
                    } else {
                        return Err(syn::Error::new(
                            expr.span(),
                            "expected identifier for entity",
                        ));
                    }
                } else {
                    return Err(syn::Error::new(meta.span(), "expected name-value pair"));
                }
            } else {
                return Err(syn::Error::new(meta.span(), "unknown sea_orm argument"));
            }
        }
        let entity =
            entity.ok_or_else(|| syn::Error::new(input.span(), "missing entity argument"))?;
        Ok(SeaOrmArgs { entity })
    }
}
