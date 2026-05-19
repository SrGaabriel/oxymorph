mod attrs;
mod expand;

use proc_macro::TokenStream;
use syn::{ItemStruct, parse_macro_input};

use crate::attrs::ModelArgs;

#[proc_macro_attribute]
pub fn model(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ModelArgs);
    let item = parse_macro_input!(item as ItemStruct);
    expand::expand_model(&args, &item).into()
}
