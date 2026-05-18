use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, ItemStruct, Token, parse::Parse, punctuated::Punctuated};

use crate::attrs::{Attr, Model, parse_attrs};

pub struct ModelArgs {
    pub variants: Vec<Model>,
}

impl Parse for ModelArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                variants: vec![Model::Delta, Model::Draft, Model::View],
            });
        }
        let list = Punctuated::<Model, Token![,]>::parse_terminated(input)?;
        Ok(Self {
            variants: list.into_iter().collect(),
        })
    }
}

pub fn expand_model(args: &ModelArgs, input: &ItemStruct) -> TokenStream {
    let mut out = emit_canonical(input);
    for variant in &args.variants {
        let v = match variant {
            Model::Delta => emit_variant(input, &Model::Delta),
            Model::Draft => emit_variant(input, &Model::Draft),
            Model::View => emit_variant(input, &Model::View),
        };
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

fn forwarded_derives(attrs: &[Attribute]) -> Vec<&Attribute> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("derive"))
        .collect()
}

fn emit_variant(input: &ItemStruct, variant: &Model) -> TokenStream {
    let suffix = match variant {
        Model::Delta => "Delta",
        Model::Draft => "Draft",
        Model::View => "View",
    };
    let struct_vis = &input.vis;
    let struct_name = suffix_name(&input.ident, suffix);
    let struct_generics = &input.generics;
    let derives = forwarded_derives(&input.attrs);

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
        match variant {
            Model::Delta => fields.push(quote! {
                #(#other)*
                #[serde(default, skip_serializing_if = "::oxymorph::Patch::is_absent")]
                #field_vis #field_name: ::oxymorph::Patch<#ty>
            }),
            Model::Draft | Model::View => fields.push(quote! {
                #(#other)*
                #field_vis #field_name: #ty
            }),
        }
    }

    quote! {
        #(#derives)*
        #struct_vis struct #struct_name #struct_generics {
            #(#fields),*
        }
    }
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
