//! `#[derive(FixedReplicatedStateFields)]` — emits the Rust adapter for

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    AngleBracketedGenericArguments, Data, DeriveInput, Field, Fields, GenericArgument, Ident,
    LitInt, PathArguments, Type,
};

struct ConstArgs {
    n_groups: TokenStream,
    n_fields_per_group: TokenStream,
    client_whitelist_size: TokenStream,
    n_user_attributes: TokenStream,
}

struct BaseField {
    ident: Ident,
    const_args: ConstArgs,
}

struct FieldInfo {
    ident: Ident,
    ty: Type,
}

struct FieldAttrs {
    skip: bool,
    group: usize,
}

pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(FixedReplicatedStateFields)] only supports structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "#[derive(FixedReplicatedStateFields)] requires named fields",
        ));
    };

    let mut base_field = None;
    let mut groups: BTreeMap<usize, Vec<FieldInfo>> = BTreeMap::new();
    for field in &fields.named {
        let Some(ident) = &field.ident else {
            continue;
        };
        let is_marked_base = field_has_fixed_state_base_attr(field)?;
        let const_args = fixed_replicated_state_const_args(&field.ty)?;
        if is_marked_base && const_args.is_none() {
            return Err(syn::Error::new_spanned(
                field,
                "#[fixed_state(base)] requires a `nw_network::hub::FixedReplicatedState<...>` field",
            ));
        }
        if let Some(const_args) = const_args {
            if base_field.is_some() {
                return Err(syn::Error::new_spanned(
                    field,
                    "only one nw_network::hub::FixedReplicatedState<...> base field is allowed",
                ));
            }
            base_field = Some(BaseField {
                ident: ident.clone(),
                const_args,
            });
            continue;
        }
        let attrs = parse_field_attrs(field)?;
        if attrs.skip {
            continue;
        }
        groups.entry(attrs.group).or_default().push(FieldInfo {
            ident: ident.clone(),
            ty: field.ty.clone(),
        });
    }

    let Some(base_field) = base_field else {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(FixedReplicatedStateFields)] requires an embedded \
             `nw_network::hub::FixedReplicatedState<...>` base field",
        ));
    };

    let ident = &input.ident;
    let base_ident = &base_field.ident;
    let const_args = &base_field.const_args;
    let n_groups = &const_args.n_groups;
    let n_fields_per_group = &const_args.n_fields_per_group;
    let client_whitelist_size = &const_args.client_whitelist_size;
    let n_user_attributes = &const_args.n_user_attributes;

    let group_count_arms = groups.iter().map(|(group_idx, fields)| {
        let field_count = field_count_expr(fields);
        quote! {
            #group_idx => {
                debug_assert!(#field_count <= #n_fields_per_group);
                ::core::option::Option::Some(#field_count)
            }
        }
    });
    let visit_arms = groups.iter().map(|(group_idx, fields)| {
        let body = expand_visit_fields(fields);
        quote! {
            #group_idx => {
                #body
            }
        }
    });
    let visit_mut_arms = groups.iter().map(|(group_idx, fields)| {
        let body = expand_visit_fields_mut(fields);
        quote! {
            #group_idx => {
                #body
                ::core::result::Result::Ok(())
            }
        }
    });
    let visit_merge_arms = groups.iter().map(|(group_idx, fields)| {
        let body = expand_visit_fields_for_merge(fields);
        quote! {
            #group_idx => {
                #body
                ::core::result::Result::Ok(())
            }
        }
    });

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::nw_network::hub::FixedReplicatedStateFields<
            #n_groups,
            #n_fields_per_group,
            #client_whitelist_size,
            #n_user_attributes,
        > for #ident #ty_generics #where_clause
        {
            fn fixed_replicated_state(
                &self,
            ) -> &::nw_network::hub::FixedReplicatedState<
                #n_groups,
                #n_fields_per_group,
                #client_whitelist_size,
                #n_user_attributes,
            > {
                &self.#base_ident
            }

            fn fixed_replicated_state_mut(
                &mut self,
            ) -> &mut ::nw_network::hub::FixedReplicatedState<
                #n_groups,
                #n_fields_per_group,
                #client_whitelist_size,
                #n_user_attributes,
            > {
                &mut self.#base_ident
            }

            fn fixed_group_field_count(&self, group_idx: usize) -> ::core::option::Option<usize> {
                match group_idx {
                    #(#group_count_arms,)*
                    _ => ::core::option::Option::None,
                }
            }

            fn visit_fixed_fields<'a>(
                &'a self,
                group_idx: usize,
                mut visit: impl FnMut(usize, &'a dyn ::nw_network::ReplicatedFieldHandlerBase),
            ) {
                match group_idx {
                    #(#visit_arms,)*
                    _ => {}
                }
            }

            fn try_visit_fixed_fields_mut(
                &mut self,
                group_idx: usize,
                mut visit: impl FnMut(
                    usize,
                    &mut dyn ::nw_network::ReplicatedFieldHandlerBase,
                ) -> ::core::result::Result<(), ::nw_network::serialize::MarshalerError>,
            ) -> ::core::result::Result<(), ::nw_network::serialize::MarshalerError> {
                match group_idx {
                    #(#visit_mut_arms,)*
                    _ => ::core::result::Result::Ok(()),
                }
            }

            fn try_visit_fixed_fields_for_merge(
                &mut self,
                old_state: &Self,
                new_state: &mut Self,
                group_idx: usize,
                mut visit: impl FnMut(
                    usize,
                    &mut dyn ::nw_network::ReplicatedFieldHandlerBase,
                    &dyn ::nw_network::ReplicatedFieldHandlerBase,
                    &mut dyn ::nw_network::ReplicatedFieldHandlerBase,
                ) -> ::core::result::Result<(), ::nw_network::serialize::MarshalerError>,
            ) -> ::core::result::Result<(), ::nw_network::serialize::MarshalerError>
            where
                Self: Sized,
            {
                match group_idx {
                    #(#visit_merge_arms,)*
                    _ => ::core::result::Result::Ok(()),
                }
            }
        }
    })
}

fn parse_field_attrs(field: &Field) -> syn::Result<FieldAttrs> {
    let mut attrs = FieldAttrs {
        skip: false,
        group: 0,
    };

    for attr in &field.attrs {
        if !attr.path().is_ident("fixed_state") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                attrs.skip = true;
                Ok(())
            } else if meta.path.is_ident("group") {
                let value = meta.value()?;
                let lit: LitInt = value.parse()?;
                attrs.group = lit.base10_parse()?;
                Ok(())
            } else {
                Err(meta.error("unsupported fixed_state field attribute"))
            }
        })?;
    }

    Ok(attrs)
}

fn field_has_fixed_state_base_attr(field: &Field) -> syn::Result<bool> {
    let mut found = false;
    for attr in &field.attrs {
        if !attr.path().is_ident("fixed_state") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("base") {
                found = true;
                Ok(())
            } else if meta.path.is_ident("skip") {
                Ok(())
            } else if meta.path.is_ident("group") {
                let value = meta.value()?;
                let _: LitInt = value.parse()?;
                Ok(())
            } else {
                Err(meta.error("unsupported fixed_state field attribute"))
            }
        })?;
    }

    Ok(found)
}

fn fixed_replicated_state_const_args(base_ty: &Type) -> syn::Result<Option<ConstArgs>> {
    let Type::Path(type_path) = base_ty else {
        return Ok(None);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != "FixedReplicatedState" {
        return Ok(None);
    }
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
        &segment.arguments
    else {
        return Err(syn::Error::new_spanned(
            base_ty,
            "FixedReplicatedState base field must include const args",
        ));
    };

    let mut const_args = Vec::new();
    for arg in args {
        match arg {
            GenericArgument::Const(_) => const_args.push(arg.to_token_stream()),
            _ => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "FixedReplicatedState generic arguments must be const arguments",
                ));
            }
        }
    }

    if const_args.len() < 2 || const_args.len() > 4 {
        return Err(syn::Error::new_spanned(
            base_ty,
            "FixedReplicatedState requires 2 to 4 const arguments",
        ));
    }

    Ok(Some(ConstArgs {
        n_groups: const_args[0].clone(),
        n_fields_per_group: const_args[1].clone(),
        client_whitelist_size: const_args.get(2).cloned().unwrap_or_else(|| quote!(0usize)),
        n_user_attributes: const_args.get(3).cloned().unwrap_or_else(|| quote!(0usize)),
    }))
}

fn field_count_expr(fields: &[FieldInfo]) -> TokenStream {
    let counts = fields.iter().map(|field| {
        let ty = &field.ty;
        quote!(<#ty as ::nw_network::hub::FixedStateRegister>::FIELD_COUNT)
    });
    quote!(0usize #(+ #counts)*)
}

fn expand_visit_fields(fields: &[FieldInfo]) -> TokenStream {
    let visits = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! {
            <#ty as ::nw_network::hub::FixedStateRegister>::visit_registered_fields(
                &self.#ident,
                first_index,
                &mut visit,
            );
            first_index += <#ty as ::nw_network::hub::FixedStateRegister>::FIELD_COUNT;
        }
    });
    quote! {
        let mut first_index = 0usize;
        #(#visits)*
        let _ = first_index;
    }
}

fn expand_visit_fields_mut(fields: &[FieldInfo]) -> TokenStream {
    let visits = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! {
            <#ty as ::nw_network::hub::FixedStateRegister>::try_visit_registered_fields_mut(
                &mut self.#ident,
                first_index,
                &mut visit,
            )?;
            first_index += <#ty as ::nw_network::hub::FixedStateRegister>::FIELD_COUNT;
        }
    });
    quote! {
        let mut first_index = 0usize;
        #(#visits)*
        let _ = first_index;
    }
}

fn expand_visit_fields_for_merge(fields: &[FieldInfo]) -> TokenStream {
    let visits = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! {
            <#ty as ::nw_network::hub::FixedStateRegister>::try_visit_registered_fields_for_merge(
                &mut self.#ident,
                &old_state.#ident,
                &mut new_state.#ident,
                first_index,
                &mut visit,
            )?;
            first_index += <#ty as ::nw_network::hub::FixedStateRegister>::FIELD_COUNT;
        }
    });
    quote! {
        let mut first_index = 0usize;
        #(#visits)*
        let _ = first_index;
    }
}
