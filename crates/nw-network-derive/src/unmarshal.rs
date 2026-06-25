use crate::attrs::{MarshalerField, MarshalerVariant};
use darling::ast::{Data, Fields};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

pub fn generate_unmarshal_body(
    ident: &syn::Ident,
    data: &Data<MarshalerVariant, MarshalerField>,
    repr: Option<&Type>,
) -> TokenStream {
    match data {
        Data::Struct(fields) => generate_struct_unmarshal(ident, fields),
        Data::Enum(variants) => generate_enum_unmarshal(ident, variants, repr),
    }
}

fn generate_struct_unmarshal(_ident: &syn::Ident, fields: &Fields<MarshalerField>) -> TokenStream {
    match fields.style {
        darling::ast::Style::Struct => {
            let field_unmarshals = fields.iter().filter_map(|f| {
                if f.skip {
                    return None;
                }

                let field_name = f.ident.as_ref().unwrap();
                let field_type = &f.ty;

                Some(if let Some(codec) = &f.codec {
                    quote! {
                        let #field_name =
                            <#codec as ::nw_network::serialize::marshaler::Codec<#field_type>>::unmarshal(rb)?;
                    }
                } else if let Some(as_type) = &f.r#as {
                    quote! {
                        let #field_name = <#field_type as ::core::convert::From<#as_type>>::from(
                            <#as_type as ::nw_network::serialize::marshaler::Marshaler>::unmarshal(rb)?,
                        );
                    }
                } else if let Some(unmarshal_with) = &f.unmarshal_with {
                    quote! {
                        let #field_name = #unmarshal_with(rb)?;
                    }
                } else {
                    quote! {
                        let #field_name =
                            <#field_type as ::nw_network::serialize::marshaler::Marshaler>::unmarshal(rb)?;
                    }
                })
            });

            let field_inits = fields.iter().map(|f| {
                let field_name = f.ident.as_ref().unwrap();
                if f.skip {
                    quote! { #field_name: ::core::default::Default::default() }
                } else {
                    quote! { #field_name }
                }
            });

            quote! {
                #(#field_unmarshals)*
                Ok(Self { #(#field_inits),* })
            }
        }
        darling::ast::Style::Tuple => {
            let field_unmarshals = fields.iter().enumerate().filter_map(|(i, f)| {
                if f.skip {
                    return None;
                }

                let field_name = quote::format_ident!("f{}", i);
                let field_type = &f.ty;

                Some(if let Some(codec) = &f.codec {
                    quote! {
                        let #field_name =
                            <#codec as ::nw_network::serialize::marshaler::Codec<#field_type>>::unmarshal(rb)?;
                    }
                } else if let Some(as_type) = &f.r#as {
                    quote! { let #field_name = <#field_type as ::core::convert::From<#as_type>>::from(<#as_type>::unmarshal(rb)?); }
                } else if let Some(unmarshal_with) = &f.unmarshal_with {
                    quote! { let #field_name = #unmarshal_with(rb)?; }
                } else {
                    quote! { let #field_name = <#field_type>::unmarshal(rb)?; }
                })
            });

            let field_values = (0..fields.len()).map(|i| {
                if fields.iter().nth(i).unwrap().skip {
                    quote! { ::core::default::Default::default() }
                } else {
                    let field_name = quote::format_ident!("f{}", i);
                    quote! { #field_name }
                }
            });

            quote! {
                #(#field_unmarshals)*
                Ok(Self(#(#field_values),*))
            }
        }
        darling::ast::Style::Unit => {
            quote! {
                Ok(Self)
            }
        }
    }
}

fn generate_enum_unmarshal(
    _ident: &syn::Ident,
    variants: &[MarshalerVariant],
    repr: Option<&Type>,
) -> TokenStream {
    if let Some(repr) = repr.filter(|_| is_direct_repr_enum(variants)) {
        return generate_direct_repr_enum_unmarshal(variants, repr);
    }

    let discriminants = enum_discriminants(variants);
    let match_arms = variants.iter().zip(discriminants).map(|(variant, discriminant)| {
        let variant_ident = &variant.ident;
        let discriminant =
            u8::try_from(discriminant).expect("enum discriminant does not fit in u8");

        match &variant.fields.style {
            darling::ast::Style::Unit => {
                quote! {
                    #discriminant => Ok(Self::#variant_ident),
                }
            }
            darling::ast::Style::Tuple => {
                let field_unmarshals = variant.fields.iter().enumerate().filter_map(|(i, f)| {
                    if f.skip {
                        return None;
                    }

                    let field_name = quote::format_ident!("f{}", i);
                    let field_type = &f.ty;

                    Some(if let Some(codec) = &f.codec {
                        quote! {
                            let #field_name =
                                <#codec as ::nw_network::serialize::marshaler::Codec<#field_type>>::unmarshal(rb)?;
                        }
                    } else if let Some(as_type) = &f.r#as {
                        quote! { let #field_name = <#field_type as ::core::convert::From<#as_type>>::from(<#as_type>::unmarshal(rb)?); }
                    } else if let Some(unmarshal_with) = &f.unmarshal_with {
                        quote! { let #field_name = #unmarshal_with(rb)?; }
                    } else {
                        quote! { let #field_name = <#field_type>::unmarshal(rb)?; }
                    })
                });

                let field_values = (0..variant.fields.len()).map(|i| {
                    if variant.fields.iter().nth(i).unwrap().skip {
                        quote! { ::core::default::Default::default() }
                    } else {
                        let field_name = quote::format_ident!("f{}", i);
                        quote! { #field_name }
                    }
                });

                quote! {
                    #discriminant => {
                        #(#field_unmarshals)*
                        Ok(Self::#variant_ident(#(#field_values),*))
                    },
                }
            }
            darling::ast::Style::Struct => {
                let field_unmarshals = variant.fields.iter().filter_map(|f| {
                    if f.skip {
                        return None;
                    }

                    let field_name = f.ident.as_ref().unwrap();
                    let field_type = &f.ty;

                    Some(if let Some(codec) = &f.codec {
                        quote! {
                            let #field_name =
                                <#codec as ::nw_network::serialize::marshaler::Codec<#field_type>>::unmarshal(rb)?;
                        }
                    } else if let Some(as_type) = &f.r#as {
                        quote! { let #field_name = <#field_type as ::core::convert::From<#as_type>>::from(<#as_type>::unmarshal(rb)?); }
                    } else if let Some(unmarshal_with) = &f.unmarshal_with {
                        quote! { let #field_name = #unmarshal_with(rb)?; }
                    } else {
                        quote! { let #field_name = <#field_type>::unmarshal(rb)?; }
                    })
                });

                let field_inits = variant.fields.iter().map(|f| {
                    let field_name = f.ident.as_ref().unwrap();
                    if f.skip {
                        quote! { #field_name: ::core::default::Default::default() }
                    } else {
                        quote! { #field_name }
                    }
                });

                quote! {
                    #discriminant => {
                        #(#field_unmarshals)*
                        Ok(Self::#variant_ident { #(#field_inits),* })
                    },
                }
            }
        }
    });

    quote! {
        let discriminant = u8::unmarshal(rb)?;
        match discriminant {
            #(#match_arms)*
            _ => Err(::nw_network::serialize::error::MarshalerError::InvalidDiscriminant { value: discriminant }),
        }
    }
}

fn generate_direct_repr_enum_unmarshal(variants: &[MarshalerVariant], repr: &Type) -> TokenStream {
    let mut unknown_arm = None;
    let discriminants = enum_discriminants(variants);
    let match_arms = variants
        .iter()
        .zip(discriminants)
        .filter_map(|(variant, discriminant)| {
            let variant_ident = &variant.ident;

            if is_unknown_variant(variant) {
                unknown_arm = Some(match &variant.fields.style {
                    darling::ast::Style::Tuple => {
                        quote! { _ => Ok(Self::#variant_ident(discriminant)), }
                    }
                    darling::ast::Style::Struct => {
                        let field_name = variant
                            .fields
                            .iter()
                            .next()
                            .and_then(|f| f.ident.as_ref())
                            .expect("unknown struct variant has a field");
                        quote! { _ => Ok(Self::#variant_ident { #field_name: discriminant }), }
                    }
                    darling::ast::Style::Unit => {
                        quote! { _ => Ok(Self::#variant_ident), }
                    }
                });
                None
            } else {
                Some(quote! {
                    value if value == (#discriminant as #repr) => Ok(Self::#variant_ident),
                })
            }
        })
        .collect::<Vec<_>>();

    let unknown_arm = unknown_arm.unwrap_or_else(|| {
        quote! {
            _ => {
                let value: u8 = ::core::convert::TryInto::try_into(discriminant).unwrap_or(u8::MAX);
                Err(::nw_network::serialize::error::MarshalerError::InvalidDiscriminant { value })
            },
        }
    });

    quote! {
        let discriminant = <#repr>::unmarshal(rb)?;
        match discriminant {
            #(#match_arms)*
            #unknown_arm
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
