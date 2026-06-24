use crate::attrs::{MarshalerField, MarshalerVariant};
use darling::ast::{Data, Fields};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Index, Type};

pub fn generate_marshal_body(
    data: &Data<MarshalerVariant, MarshalerField>,
    repr: Option<&Type>,
) -> TokenStream {
    match data {
        Data::Struct(fields) => generate_struct_marshal(fields),
        Data::Enum(variants) => generate_enum_marshal(variants, repr),
    }
}

fn generate_struct_marshal(fields: &Fields<MarshalerField>) -> TokenStream {
    match fields.style {
        darling::ast::Style::Struct => {
            let field_marshals = fields.iter().filter_map(|f| {
                if f.skip {
                    return None;
                }

                let field_name = f.ident.as_ref().unwrap();

                Some(if let Some(as_type) = &f.r#as {
                    quote! { <#as_type as ::core::convert::From<_>>::from(self.#field_name.clone()).marshal(wb); }
                } else if let Some(with) = &f.with {
                    quote! { #with(&self.#field_name, wb); }
                } else {
                    quote! { self.#field_name.marshal(wb); }
                })
            });

            quote! {
                #(#field_marshals)*
            }
        }
        darling::ast::Style::Tuple => {
            let field_marshals = fields.iter().enumerate().filter_map(|(i, f)| {
                if f.skip {
                    return None;
                }

                let index = Index::from(i);

                Some(if let Some(as_type) = &f.r#as {
                    quote! { <#as_type as ::core::convert::From<_>>::from(self.#index.clone()).marshal(wb); }
                } else if let Some(with) = &f.with {
                    quote! { #with(&self.#index, wb); }
                } else {
                    quote! { self.#index.marshal(wb); }
                })
            });

            quote! {
                #(#field_marshals)*
            }
        }
        darling::ast::Style::Unit => {
            quote! {
                // Unit struct - nothing to marshal
            }
        }
    }
}

fn generate_enum_marshal(variants: &[MarshalerVariant], repr: Option<&Type>) -> TokenStream {
    if let Some(repr) = repr.filter(|_| is_direct_repr_enum(variants)) {
        return generate_direct_repr_enum_marshal(variants, repr);
    }

    let discriminants = enum_discriminants(variants);
    let match_arms = variants.iter().zip(discriminants).map(|(variant, discriminant)| {
        let variant_ident = &variant.ident;

        match &variant.fields.style {
            darling::ast::Style::Unit => {
                quote! {
                    Self::#variant_ident => {
                        (#discriminant as u8).marshal(wb);
                    }
                }
            }
            darling::ast::Style::Tuple => {
                let field_names: Vec<_> = (0..variant.fields.len())
                    .map(|i| quote::format_ident!("f{}", i))
                    .collect();

                let field_marshals = variant.fields.iter().zip(&field_names).filter_map(|(f, name)| {
                    if f.skip {
                        return None;
                    }

                    Some(if let Some(as_type) = &f.r#as {
                        quote! { <#as_type as ::core::convert::From<_>>::from(#name.clone()).marshal(wb); }
                    } else if let Some(with) = &f.with {
                        quote! { #with(#name, wb); }
                    } else {
                        quote! { #name.marshal(wb); }
                    })
                });

                quote! {
                    Self::#variant_ident(#(#field_names),*) => {
                        (#discriminant as u8).marshal(wb);
                        #(#field_marshals)*
                    }
                }
            }
            darling::ast::Style::Struct => {
                let field_names: Vec<_> = variant.fields.iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();

                let field_marshals = variant.fields.iter().filter_map(|f| {
                    if f.skip {
                        return None;
                    }

                    let name = f.ident.as_ref().unwrap();

                    Some(if let Some(as_type) = &f.r#as {
                        quote! { <#as_type as ::core::convert::From<_>>::from(#name.clone()).marshal(wb); }
                    } else if let Some(with) = &f.with {
                        quote! { #with(#name, wb); }
                    } else {
                        quote! { #name.marshal(wb); }
                    })
                });

                quote! {
                    Self::#variant_ident { #(#field_names),* } => {
                        (#discriminant as u8).marshal(wb);
                        #(#field_marshals)*
                    }
                }
            }
        }
    });

    quote! {
        match self {
            #(#match_arms)*
        }
    }
}

fn generate_direct_repr_enum_marshal(variants: &[MarshalerVariant], repr: &Type) -> TokenStream {
    let discriminants = enum_discriminants(variants);
    let match_arms = variants
        .iter()
        .zip(discriminants)
        .map(|(variant, discriminant)| {
            let variant_ident = &variant.ident;

            if is_unknown_variant(variant) {
                match &variant.fields.style {
                    darling::ast::Style::Tuple => {
                        quote! {
                            Self::#variant_ident(raw) => {
                                raw.marshal(wb);
                            }
                        }
                    }
                    darling::ast::Style::Struct => {
                        let field_name = variant
                            .fields
                            .iter()
                            .next()
                            .and_then(|f| f.ident.as_ref())
                            .expect("unknown struct variant has a field");
                        quote! {
                            Self::#variant_ident { #field_name } => {
                                #field_name.marshal(wb);
                            }
                        }
                    }
                    darling::ast::Style::Unit => {
                        quote! {
                            Self::#variant_ident => {
                                let value: #repr = #discriminant as #repr;
                                value.marshal(wb);
                            }
                        }
                    }
                }
            } else {
                quote! {
                    Self::#variant_ident => {
                        let value: #repr = #discriminant as #repr;
                        value.marshal(wb);
                    }
                }
            }
        });

    quote! {
        match self {
            #(#match_arms)*
        }
    }
}

fn enum_discriminants(variants: &[MarshalerVariant]) -> Vec<u64> {
    let mut next = 0_u64;
    variants
        .iter()
        .map(|variant| {
            let value = variant.discriminant_value(next);
            next = value.saturating_add(1);
            value
        })
        .collect()
}

fn is_unknown_variant(variant: &MarshalerVariant) -> bool {
    variant.unknown || variant.ident == "Unknown"
}

fn is_direct_repr_enum(variants: &[MarshalerVariant]) -> bool {
    variants.iter().all(|variant| {
        matches!(variant.fields.style, darling::ast::Style::Unit)
            || (is_unknown_variant(variant) && variant.fields.len() == 1)
    })
}
