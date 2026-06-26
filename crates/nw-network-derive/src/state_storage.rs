use syn::{Fields, Ident, ItemStruct, Type, parse_quote};

pub fn fixed_replicated_state(
    args: proc_macro2::TokenStream,
    input: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    if args.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "fixed_replicated_state requires the base field type",
        ));
    }

    let base_ty = syn::parse2::<Type>(args)?;
    let mut item = syn::parse2::<ItemStruct>(input)?;
    inject_named_field(
        &mut item,
        Ident::new("base", proc_macro2::Span::call_site()),
        parse_quote! {
            pub(crate) base: #base_ty
        },
    )?;

    Ok(quote::quote!(#item))
}

pub(crate) fn inject_named_field(
    item: &mut ItemStruct,
    field_ident: Ident,
    field: syn::Field,
) -> syn::Result<()> {
    let Fields::Named(fields) = &mut item.fields else {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "base injection only supports structs with named fields",
        ));
    };

    if fields
        .named
        .iter()
        .any(|field| field.ident.as_ref() == Some(&field_ident))
    {
        return Err(syn::Error::new_spanned(
            field_ident,
            "base field is already declared",
        ));
    }

    fields.named.push(field);
    Ok(())
}
