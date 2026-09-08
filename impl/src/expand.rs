use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, ItemStruct, Meta, Token, punctuated::Punctuated};

use crate::attrs::{FieldAttrs, Model, ModelArgs, Variant, parse_field_attrs};

const TYPE_SENSITIVE_SERDE_KEYS: &[&str] =
    &["default", "with", "serialize_with", "deserialize_with"];

pub fn expand_model(args: &ModelArgs, input: &ItemStruct) -> TokenStream {
    match try_expand(args, input) {
        Ok(ts) => ts,
        Err(e) => {
            let mut out = emit_canonical(input);
            out.extend(e.to_compile_error());
            out
        }
    }
}

fn try_expand(args: &ModelArgs, input: &ItemStruct) -> syn::Result<TokenStream> {
    let fields = input
        .fields
        .iter()
        .map(|f| parse_field_attrs(f).map(|a| (f, a)))
        .collect::<syn::Result<Vec<_>>>()?;

    if args.sea_orm.is_none()
        && let Some((f, _)) = fields
            .iter()
            .find(|(_, a)| a.sea_orm.column.is_some() || a.sea_orm.with.is_some())
    {
        return Err(syn::Error::new_spanned(
            f,
            "field-level sea_orm(...) requires sea_orm(entity = ...) on the struct",
        ));
    }

    let mut out = emit_canonical(input);
    for variant in &args.variants {
        out.extend(emit_variant(input, args, variant, &fields)?);
    }
    Ok(out)
}

fn emit_canonical(input: &ItemStruct) -> TokenStream {
    let attrs = &input.attrs;
    let vis = &input.vis;
    let name = &input.ident;
    let generics = &input.generics;
    let (_, _, where_clause) = generics.split_for_impl();

    let fields = input.fields.iter().map(|f| {
        let kept = f.attrs.iter().filter(|a| !a.path().is_ident("oxymorph"));
        let vis = &f.vis;
        let ident = &f.ident;
        let ty = &f.ty;
        quote! { #(#kept)* #vis #ident: #ty }
    });

    quote! {
        #(#attrs)*
        #vis struct #name #generics #where_clause {
            #(#fields),*
        }
    }
}

#[derive(Default)]
struct Derives {
    ser: bool,
    de: bool,
    schema: bool,
}

fn detect_derives<'a>(attrs: impl IntoIterator<Item = &'a Attribute>) -> syn::Result<Derives> {
    let mut d = Derives::default();
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        attr.parse_nested_meta(|m| {
            if let Some(last) = m.path.segments.last() {
                match last.ident.to_string().as_str() {
                    "Serialize" => d.ser = true,
                    "Deserialize" => d.de = true,
                    "ToSchema" => d.schema = true,
                    _ => {}
                }
            }
            Ok(())
        })?;
    }
    Ok(d)
}

fn attr_keys(attr: &Attribute) -> Vec<String> {
    attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .map(|metas| {
            metas
                .iter()
                .filter_map(|m| m.path().get_ident().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn has_key<'a>(attrs: impl IntoIterator<Item = &'a Attribute>, path: &str, key: &str) -> bool {
    attrs
        .into_iter()
        .filter(|a| a.path().is_ident(path))
        .any(|a| attr_keys(a).iter().any(|k| k == key))
}

fn strip_serde_for_delta(attr: &Attribute) -> Option<Attribute> {
    if !attr.path().is_ident("serde") {
        return Some(attr.clone());
    }
    let Ok(metas) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Some(attr.clone());
    };
    let kept: Punctuated<Meta, Token![,]> = metas
        .into_iter()
        .filter(|m| {
            !m.path()
                .get_ident()
                .is_some_and(|i| TYPE_SENSITIVE_SERDE_KEYS.contains(&i.to_string().as_str()))
        })
        .collect();
    if kept.is_empty() {
        None
    } else {
        Some(syn::parse_quote!(#[serde(#kept)]))
    }
}

fn field_attrs_for(
    model: Model,
    ty: &syn::Type,
    attrs: &FieldAttrs,
    derives: &Derives,
) -> Vec<Attribute> {
    let mut out: Vec<Attribute> = match model {
        Model::Delta => attrs
            .other
            .iter()
            .filter_map(strip_serde_for_delta)
            .collect(),
        Model::Draft | Model::View => attrs.other.clone(),
    };
    out.extend(attrs.scoped_for(model).cloned());

    if model == Model::Delta {
        let mut serde_parts: Vec<TokenStream> = Vec::new();
        if derives.de && !has_key(&out, "serde", "default") {
            serde_parts.push(quote! { default });
        }
        if derives.ser && !has_key(&out, "serde", "skip_serializing_if") {
            serde_parts.push(quote! { skip_serializing_if = "::oxymorph::Patch::is_absent" });
        }
        if !serde_parts.is_empty() {
            out.push(syn::parse_quote!(#[serde(#(#serde_parts),*)]));
        }
        if derives.schema {
            let mut schema_parts: Vec<TokenStream> = Vec::new();
            if !has_key(&out, "schema", "value_type") {
                schema_parts.push(quote! { value_type = #ty });
            }
            if !has_key(&out, "schema", "required") {
                schema_parts.push(quote! { required = false });
            }
            if !schema_parts.is_empty() {
                out.push(syn::parse_quote!(#[schema(#(#schema_parts),*)]));
            }
        }
    }
    out
}

fn emit_variant(
    input: &ItemStruct,
    args: &ModelArgs,
    variant: &Variant,
    fields: &[(&syn::Field, FieldAttrs)],
) -> syn::Result<TokenStream> {
    let model = variant.model;
    let struct_vis = &input.vis;
    let struct_name = variant
        .name
        .clone()
        .unwrap_or_else(|| suffix_name(&input.ident, model.suffix()));
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let struct_attrs: Vec<&Attribute> = input.attrs.iter().chain(&variant.attrs).collect();
    let derives = detect_derives(struct_attrs.iter().copied())?;

    let mut included = Vec::new();
    let mut field_tokens = Vec::new();
    for (field, attrs) in fields {
        if !attrs.included_in(model) {
            continue;
        }
        let ty = &field.ty;
        let field_attrs = field_attrs_for(model, ty, attrs, &derives);
        let field_name = field.ident.as_ref().unwrap();
        let field_vis = &field.vis;
        let ty = match model {
            Model::Delta => quote! { ::oxymorph::Patch<#ty> },
            Model::Draft | Model::View => quote! { #ty },
        };
        field_tokens.push(quote! {
            #(#field_attrs)*
            #field_vis #field_name: #ty
        });
        included.push((*field, attrs));
    }

    let mut current = quote! {
        #(#struct_attrs)*
        #struct_vis struct #struct_name #generics #where_clause {
            #(#field_tokens),*
        }
    };

    if model == Model::Delta
        && let Some(sea_orm) = args.sea_orm.as_ref()
    {
        let entity = &sea_orm.entity;
        let assignments = included.iter().map(|(field, attrs)| {
            let name = field.ident.as_ref().unwrap();
            let column = attrs.sea_orm.column.as_ref().unwrap_or(name);
            let value = match (&attrs.sea_orm.with, &sea_orm.error) {
                (Some(f), Some(_)) => quote! { #f(value)? },
                (Some(f), None) => quote! { #f(value) },
                (None, _) => quote! { ::core::convert::Into::into(value) },
            };
            quote! {
                if let ::oxymorph::Patch::Set(value) = self.#name {
                    entity.#column = ::sea_orm::ActiveValue::Set(#value);
                }
            }
        });
        let (ret, tail) = match &sea_orm.error {
            Some(err) => (
                quote! { -> ::core::result::Result<(), #err> },
                quote! { ::core::result::Result::Ok(()) },
            ),
            None => (quote! {}, quote! {}),
        };
        current.extend(quote! {
            impl #impl_generics #struct_name #ty_generics #where_clause {
                pub fn apply_to(self, entity: &mut #entity::ActiveModel) #ret {
                    #(#assignments)*
                    #tail
                }
            }
        });
    }

    Ok(current)
}

fn suffix_name(name: &syn::Ident, suffix: &str) -> syn::Ident {
    syn::Ident::new(&format!("{name}{suffix}"), name.span())
}
