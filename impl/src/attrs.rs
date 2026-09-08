use syn::{
    Attribute, Meta, Token,
    parse::{Parse, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Model {
    Delta,
    Draft,
    View,
}

impl Model {
    pub fn from_path(path: &syn::Path) -> Option<Self> {
        if path.is_ident("delta") {
            Some(Model::Delta)
        } else if path.is_ident("draft") {
            Some(Model::Draft)
        } else if path.is_ident("view") {
            Some(Model::View)
        } else {
            None
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Model::Delta => "Delta",
            Model::Draft => "Draft",
            Model::View => "View",
        }
    }
}

#[derive(Default)]
pub(crate) struct ScopeBody {
    pub skip: bool,
    pub name: Option<syn::Ident>,
    pub attrs: Vec<Attribute>,
}

fn parse_scope_body(tokens: proc_macro2::TokenStream) -> syn::Result<ScopeBody> {
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(tokens)?;
    let mut body = ScopeBody::default();
    for meta in metas {
        if meta.path().is_ident("skip") {
            match meta {
                Meta::Path(_) => body.skip = true,
                _ => return Err(syn::Error::new(meta.span(), "expected bare `skip`")),
            }
        } else if meta.path().is_ident("name") {
            body.name = Some(parse_ident_value(&meta)?);
        } else if meta.path().is_ident("attr") {
            let Meta::List(list) = meta else {
                return Err(syn::Error::new(meta.span(), "expected attr(...)"));
            };
            let inner = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens)?;
            body.attrs.extend(inner.iter().map(meta_to_attr));
        } else {
            body.attrs.push(meta_to_attr(&meta));
        }
    }
    Ok(body)
}

fn meta_to_attr(meta: &Meta) -> Attribute {
    syn::parse_quote!(#[#meta])
}

fn parse_ident_value(meta: &Meta) -> syn::Result<syn::Ident> {
    let Meta::NameValue(nv) = meta else {
        return Err(syn::Error::new(meta.span(), "expected `name = Ident`"));
    };
    match &nv.value {
        syn::Expr::Path(p) => p
            .path
            .get_ident()
            .cloned()
            .ok_or_else(|| syn::Error::new(nv.value.span(), "expected an identifier")),
        other => Err(syn::Error::new(other.span(), "expected an identifier")),
    }
}

fn parse_path_value(meta: &Meta) -> syn::Result<syn::Path> {
    let Meta::NameValue(nv) = meta else {
        return Err(syn::Error::new(meta.span(), "expected `key = path`"));
    };
    match &nv.value {
        syn::Expr::Path(p) => Ok(p.path.clone()),
        other => Err(syn::Error::new(other.span(), "expected a path")),
    }
}

fn parse_type_value(meta: &Meta) -> syn::Result<syn::Type> {
    let Meta::NameValue(nv) = meta else {
        return Err(syn::Error::new(meta.span(), "expected `key = Type`"));
    };
    match &nv.value {
        syn::Expr::Path(p) => Ok(syn::Type::Path(syn::TypePath {
            qself: p.qself.clone(),
            path: p.path.clone(),
        })),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => s.parse(),
        other => Err(syn::Error::new(other.span(), "expected a type")),
    }
}

#[derive(Default)]
pub(crate) struct FieldSeaOrm {
    pub column: Option<syn::Ident>,
    pub with: Option<syn::Path>,
}

#[derive(Default)]
pub(crate) struct FieldAttrs {
    pub skip: Vec<Model>,
    pub scoped: Vec<(Model, Attribute)>,
    pub sea_orm: FieldSeaOrm,
    pub other: Vec<Attribute>,
}

impl FieldAttrs {
    pub fn included_in(&self, model: Model) -> bool {
        !self.skip.contains(&model)
    }

    pub fn scoped_for(&self, model: Model) -> impl Iterator<Item = &Attribute> {
        self.scoped
            .iter()
            .filter(move |(m, _)| *m == model)
            .map(|(_, a)| a)
    }
}

pub(crate) fn parse_field_attrs(field: &syn::Field) -> syn::Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    for attr in &field.attrs {
        if !attr.path().is_ident("oxymorph") {
            out.other.push(attr.clone());
            continue;
        }
        let metas = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in metas {
            let path = meta.path();
            if let Some(model) = Model::from_path(path) {
                let Meta::List(list) = meta else {
                    return Err(syn::Error::new(
                        meta.span(),
                        format!("expected {}(...)", path.get_ident().unwrap()),
                    ));
                };
                let body = parse_scope_body(list.tokens)?;
                if let Some(name) = body.name {
                    return Err(syn::Error::new(
                        name.span(),
                        "`name` is only valid at the struct level",
                    ));
                }
                if body.skip {
                    out.skip.push(model);
                }
                out.scoped
                    .extend(body.attrs.into_iter().map(|a| (model, a)));
            } else if path.is_ident("read_only") {
                out.skip.extend([Model::Delta, Model::Draft]);
            } else if path.is_ident("write_only") {
                out.skip.push(Model::View);
            } else if path.is_ident("create_only") {
                out.skip.push(Model::Delta);
            } else if path.is_ident("sea_orm") {
                let Meta::List(list) = meta else {
                    return Err(syn::Error::new(meta.span(), "expected sea_orm(...)"));
                };
                let inner = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens)?;
                for m in inner {
                    if m.path().is_ident("column") {
                        out.sea_orm.column = Some(parse_ident_value(&m)?);
                    } else if m.path().is_ident("with") {
                        out.sea_orm.with = Some(parse_path_value(&m)?);
                    } else {
                        return Err(syn::Error::new(
                            m.span(),
                            "unknown sea_orm argument, expected `column` or `with`",
                        ));
                    }
                }
            } else {
                return Err(syn::Error::new(
                    meta.span(),
                    "unknown oxymorph attribute, expected one of: read_only, write_only, create_only, \
                     sea_orm(...), delta(...), draft(...), view(...)",
                ));
            }
        }
    }
    Ok(out)
}

pub(crate) struct Variant {
    pub model: Model,
    pub name: Option<syn::Ident>,
    pub attrs: Vec<Attribute>,
}

pub(crate) struct ModelArgs {
    pub variants: Vec<Variant>,
    pub sea_orm: Option<SeaOrmArgs>,
}

pub(crate) struct SeaOrmArgs {
    pub entity: syn::Path,
    pub error: Option<syn::Type>,
}

impl Parse for ModelArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "expected at least one of: delta, draft, view",
            ));
        }
        let mut variants: Vec<Variant> = Vec::new();
        let mut sea_orm = None;
        let punctuated = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in punctuated {
            let path = meta.path();
            if let Some(model) = Model::from_path(path) {
                if variants.iter().any(|v| v.model == model) {
                    return Err(syn::Error::new(meta.span(), "duplicate model"));
                }
                let body = match &meta {
                    Meta::Path(_) => ScopeBody::default(),
                    Meta::List(list) => parse_scope_body(list.tokens.clone())?,
                    Meta::NameValue(_) => {
                        return Err(syn::Error::new(
                            meta.span(),
                            format!("expected `{0}` or `{0}(...)`", path.get_ident().unwrap()),
                        ));
                    }
                };
                if body.skip {
                    return Err(syn::Error::new(
                        meta.span(),
                        "`skip` is only valid at the field level",
                    ));
                }
                variants.push(Variant {
                    model,
                    name: body.name,
                    attrs: body.attrs,
                });
            } else if path.is_ident("sea_orm") {
                let Meta::List(list) = meta else {
                    return Err(syn::Error::new(meta.span(), "expected sea_orm(...)"));
                };
                sea_orm = Some(SeaOrmArgs::parse.parse2(list.tokens)?);
            } else {
                return Err(syn::Error::new(
                    meta.span(),
                    "unknown model argument, expected one of: delta, draft, view, sea_orm(...)",
                ));
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
        let punctuated = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        let mut entity = None;
        let mut error = None;
        for meta in punctuated {
            if meta.path().is_ident("entity") {
                entity = Some(parse_path_value(&meta)?);
            } else if meta.path().is_ident("error") {
                error = Some(parse_type_value(&meta)?);
            } else {
                return Err(syn::Error::new(
                    meta.span(),
                    "unknown sea_orm argument, expected `entity` or `error`",
                ));
            }
        }
        let entity =
            entity.ok_or_else(|| syn::Error::new(input.span(), "missing entity argument"))?;
        Ok(SeaOrmArgs { entity, error })
    }
}
