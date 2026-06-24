use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            ident,
            "fragment registration does not support generic types",
        ));
    }

    Ok(quote! {
        ::inventory::submit! {
            ::nw_network::hub::FragmentRegistration::of::<#ident>()
        }
    })
}
