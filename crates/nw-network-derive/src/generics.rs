use syn::{GenericParam, Generics, Path};

pub fn add_marshal_bounds(generics: &Generics) -> Generics {
    add_bounds(
        generics,
        &syn::parse_quote!(::nw_network::serialize::marshaler::Marshal),
    )
}

pub fn add_unmarshal_bounds(generics: &Generics) -> Generics {
    add_bounds(
        generics,
        &syn::parse_quote!(::nw_network::serialize::marshaler::Unmarshal),
    )
}

fn add_bounds(generics: &Generics, bound: &Path) -> Generics {
    let mut generics = generics.clone();

    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(syn::parse_quote!(#bound));
        }
    }

    generics
}
