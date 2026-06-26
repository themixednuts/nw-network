use syn::{GenericParam, Generics};

/// Add `Marshaler` trait bound to all type parameters in the generics
pub fn add_marshaler_bounds(generics: &Generics) -> Generics {
    let mut generics = generics.clone();

    // Add Marshaler bound to each type parameter
    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(syn::parse_quote!(
                ::nw_network::serialize::marshaler::Marshaler
            ));
        }
    }

    generics
}
