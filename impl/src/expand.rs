use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, ItemStruct};

use crate::attrs::{Attr, Model, ModelArgs, parse_attrs};

pub fn expand_model(args: &ModelArgs, input: &ItemStruct) -> TokenStream {
    let mut out = emit_canonical(input);
    for variant in &args.variants {
        let v = emit_variant(input, args, variant);
        out.extend(v);
    }
    out
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

struct ModelDerives<'a> {
    derives: Vec<&'a Attribute>,
    ser: bool,
    de: bool,
}

fn parse_derives(attrs: &[Attribute]) -> ModelDerives<'_> {
    let mut ser = false;
    let mut de = false;
    let derives = attrs
        .iter()
        .filter(|a| a.path().is_ident("derive"))
        .inspect(|a| {
            a.parse_nested_meta(|m| {
                let last = m.path.segments.last();
                if last.is_some_and(|s| s.ident == "Serialize") {
                    ser = true;
                }
                if last.is_some_and(|s| s.ident == "Deserialize") {
                    de = true;
                }
                Ok(())
            })
            .unwrap();
        })
        .collect();
    ModelDerives { derives, ser, de }
}

fn emit_variant(input: &ItemStruct, args: &ModelArgs, variant: &Model) -> TokenStream {
    let suffix = match variant {
        Model::Delta => "Delta",
        Model::Draft => "Draft",
        Model::View => "View",
    };
    let struct_vis = &input.vis;
    let struct_name = suffix_name(&input.ident, suffix);
    let struct_generics = &input.generics;
    let parsed_derives = parse_derives(&input.attrs);
    let derives = parsed_derives.derives;

    let mut field_names = Vec::new();
    let mut fields = Vec::new();
    for field in &input.fields {
        let field_attrs = match parse_attrs(field) {
            Ok(a) => a,
            Err(e) => return e.to_compile_error(),
        };
        if !field_included(variant, &field_attrs.oxymorph) {
            continue;
        }
        let ty = &field.ty;
        let other = &field_attrs.other;
        let field_name = field.ident.as_ref().unwrap();
        let field_vis = &field.vis;
        field_names.push(field_name.clone());
        match variant {
            Model::Delta => {
                let serde_attr = match (parsed_derives.ser, parsed_derives.de) {
                    (true, true) => {
                        quote! { #[serde(default, skip_serializing_if = "::oxymorph::Patch::is_absent")] }
                    }
                    (true, false) => {
                        quote! { #[serde(skip_serializing_if = "::oxymorph::Patch::is_absent")] }
                    }
                    (false, true) => quote! { #[serde(default)] },
                    (false, false) => quote! {},
                };
                fields.push(quote! {
                    #(#other)*
                    #serde_attr
                    #field_vis #field_name: ::oxymorph::Patch<#ty>
                });
            }
            Model::Draft | Model::View => fields.push(quote! {
                #(#other)*
                #field_vis #field_name: #ty
            }),
        }
    }

    let mut current = quote! {
        #(#derives)*
        #struct_vis struct #struct_name #struct_generics {
            #(#fields),*
        }
    };
    if matches!(variant, Model::Delta)
        && let Some(sea_orm) = args.sea_orm.as_ref()
    {
        let sea_orm_entity_path = &sea_orm.entity;
        current.extend(quote! {
            impl #struct_name {
                pub fn apply_to(self, entity: &mut #sea_orm_entity_path::ActiveModel) {
                    #(
                        if let ::oxymorph::Patch::Set(value) = self.#field_names {
                            entity.#field_names = ::sea_orm::ActiveValue::Set(value.into());
                        }
                     )*
                }
            }
        });
    }

    current
}

fn field_included(variant: &Model, attrs: &[Attr]) -> bool {
    for a in attrs {
        match (variant, a) {
            (Model::Delta, Attr::Immutable | Attr::ServerOnly | Attr::Hide(Model::Delta))
            | (Model::Draft, Attr::ServerOnly | Attr::Hide(Model::Draft))
            | (Model::View, Attr::Hide(Model::View)) => return false,
            _ => {}
        }
    }
    true
}

fn suffix_name(name: &syn::Ident, suffix: &str) -> syn::Ident {
    syn::Ident::new(&format!("{name}{suffix}"), name.span())
}
