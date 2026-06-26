use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields, Ident, LitInt, LitStr, Type};

struct Opts {
    category: Option<String>,
    world_position: Option<String>,
}

struct FieldInfo {
    ident: Ident,
    name: LitStr,
    ty: Type,
}

struct BaseField {
    ident: Ident,
}

pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let opts = parse_opts(input)?;

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(ReplicatedState)] only supports structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "#[derive(ReplicatedState)] requires named fields",
        ));
    };

    let mut groups: BTreeMap<usize, Vec<FieldInfo>> = BTreeMap::new();
    for field in &fields.named {
        if field_is_base(field) {
            continue;
        }
        if field_is_skipped(field) {
            continue;
        }
        let Some(ident) = &field.ident else {
            continue;
        };
        let attrs = parse_field_attrs(field)?;
        groups.entry(attrs.group).or_default().push(FieldInfo {
            ident: ident.clone(),
            name: attrs.name,
            ty: field.ty.clone(),
        });
    }

    if groups.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(ReplicatedState)] requires at least one field",
        ));
    }

    let ident = &input.ident;
    let base_field = find_base_field(ident, fields.named.iter())?;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let contents_impl =
        expand_generated_contents(ident, &groups, &impl_generics, &ty_generics, where_clause);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let fragment_impl = expand_fragment(
        ident,
        &base_field,
        &groups,
        &opts,
        &impl_generics,
        &ty_generics,
        where_clause,
    )?;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let marshaler_impl = quote! {
        impl #impl_generics ::nw_network::serialize::marshaler::Marshaler
        for #ident #ty_generics #where_clause
        {
            fn marshal(&self, wb: &mut ::nw_network::serialize::buffer::WriteBuffer) {
                ::nw_network::hub::DynFragment::marshal_contents(self, wb);
            }

            fn unmarshal(
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<Self, ::nw_network::serialize::error::MarshalerError> {
                let mut value = Self::default();
                if !rb.is_empty() {
                    ::nw_network::hub::DynFragment::unmarshal_contents(&mut value, rb)?;
                }
                Ok(value)
            }
        }
    };
    let fragment_registration = expand_fragment_registration(input, ident)?;
    Ok(quote! {
        #contents_impl
        #fragment_impl
        #marshaler_impl
        #fragment_registration
    })
}

fn parse_opts(input: &DeriveInput) -> syn::Result<Opts> {
    let mut opts = Opts {
        category: None,
        world_position: None,
    };

    for attr in &input.attrs {
        if !attr.path().is_ident("replicated_state") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("facet") {
                Err(meta.error(
                    "ReplicatedState does not own facet metadata; this derive emits fragment and marshaler glue only",
                ))
            } else if meta.path.is_ident("message") {
                Err(meta.error(
                    "`message` is unsupported on ReplicatedState; this derive emits fragment and marshaler glue only",
                ))
            } else if meta.path.is_ident("category") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                opts.category = Some(lit.value());
                Ok(())
            } else if meta.path.is_ident("world_position") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                opts.world_position = Some(lit.value());
                Ok(())
            } else {
                Err(meta.error("unsupported replicated_state attribute"))
            }
        })?;
    }

    Ok(opts)
}

fn field_is_skipped(field: &syn::Field) -> bool {
    field_has_attr(field, "skip")
}

fn field_is_base(field: &Field) -> bool {
    let Type::Path(type_path) = &field.ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "ReplicatedState")
}

fn expand_fragment_registration(input: &DeriveInput, ident: &Ident) -> syn::Result<TokenStream> {
    let has_type_registry = input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("type_registry"));
    if !has_type_registry {
        return Ok(quote! {});
    }
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            ident,
            "fragment registration from #[derive(ReplicatedState)] does not support generic types",
        ));
    }

    Ok(quote! {
        ::inventory::submit! {
            ::nw_network::hub::FragmentRegistration::of::<#ident>()
        }
    })
}

fn field_has_attr(field: &Field, name: &str) -> bool {
    for attr in &field.attrs {
        if !attr.path().is_ident("replicated_state") {
            continue;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(name) {
                found = true;
                Ok(())
            } else if meta.path.is_ident("group") {
                let value = meta.value()?;
                let _: LitInt = value.parse()?;
                Ok(())
            } else if meta.path.is_ident("name") {
                let value = meta.value()?;
                let _: LitStr = value.parse()?;
                Ok(())
            } else {
                Err(meta.error("unsupported replicated_state field attribute"))
            }
        });
        if found {
            return true;
        }
    }
    false
}

fn find_base_field<'a, I>(ident: &Ident, fields: I) -> syn::Result<BaseField>
where
    I: Iterator<Item = &'a Field>,
{
    let mut base = None;
    for field in fields {
        if !field_is_base(field) {
            continue;
        }
        let Some(field_ident) = &field.ident else {
            continue;
        };
        if base.is_some() {
            return Err(syn::Error::new_spanned(
                field,
                "only one nw_network::hub::ReplicatedState base field is allowed",
            ));
        }
        base = Some(BaseField {
            ident: field_ident.clone(),
        });
    }

    let Some(base_field) = base else {
        return Err(syn::Error::new_spanned(
            ident,
            "#[derive(ReplicatedState)] requires an embedded \
             `nw_network::hub::ReplicatedState` base field",
        ));
    };

    Ok(base_field)
}

struct FieldAttrs {
    group: usize,
    name: LitStr,
}

fn parse_field_attrs(field: &syn::Field) -> syn::Result<FieldAttrs> {
    let mut group = 0usize;
    let ident = field
        .ident
        .as_ref()
        .expect("ReplicatedState named field checked before attr parse");
    let mut name = LitStr::new(&ident.to_string(), ident.span());

    for attr in &field.attrs {
        if !attr.path().is_ident("replicated_state") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("group") {
                let value = meta.value()?;
                let lit: LitInt = value.parse()?;
                group = lit.base10_parse()?;
                Ok(())
            } else if meta.path.is_ident("name") {
                let value = meta.value()?;
                name = value.parse()?;
                Ok(())
            } else if meta.path.is_ident("skip") {
                Ok(())
            } else {
                Err(meta.error("unsupported replicated_state field attribute"))
            }
        })?;
    }

    Ok(FieldAttrs { group, name })
}

fn expand_generated_contents(
    ident: &Ident,
    groups: &BTreeMap<usize, Vec<FieldInfo>>,
    impl_generics: &syn::ImplGenerics<'_>,
    ty_generics: &syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
) -> TokenStream {
    let max_descriptor = groups.keys().copied().max().unwrap_or(0);
    let descriptor_count = max_descriptor + 1;
    let unmarshal_group = |group_idx: &usize, fields: &Vec<FieldInfo>| {
        let field_count = fields.len();
        let field_unmarshal = fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! {
                if !descriptor_done {
                    if field_index % 7 == 0 {
                        field_mask = rb.read_u8()?;
                    }
                    if (field_mask & (1 << (field_index % 7))) != 0 {
                        self.#ident =
                            <#ty as ::nw_network::serialize::marshaler::Marshaler>::unmarshal(rb)?;
                    }
                    if (field_index % 7 == 6 || field_index + 1 == field_count)
                        && (field_mask & 0x80) == 0
                    {
                        descriptor_done = true;
                    }
                }
                field_index += 1;
            }
        });
        let group_bit = 1u8 << (group_idx % 8);

        quote! {
            if (descriptor_mask & #group_bit) != 0 {
                let field_count = #field_count;
                let mut field_mask = 0u8;
                let mut field_index = 0usize;
                let mut descriptor_done = false;
                #(#field_unmarshal)*
                if field_count != 0 && field_count % 7 == 0 && !descriptor_done {
                    loop {
                        let mask = rb.read_u8()?;
                        if (mask & 0x80) == 0 {
                            break;
                        }
                    }
                }
                let _ = (field_index, descriptor_done);
            }
        }
    };
    let unmarshal_chunks = (0..=(max_descriptor / 8)).map(|chunk_idx| {
        let chunk_start = chunk_idx * 8;
        let chunk_end = chunk_start + 8;
        let groups_in_chunk = groups
            .iter()
            .filter(|(group_idx, _)| **group_idx >= chunk_start && **group_idx < chunk_end)
            .map(|(group_idx, fields)| unmarshal_group(group_idx, fields));
        quote! {
            let descriptor_mask = rb.read_u8()?;
            #(#groups_in_chunk)*
        }
    });

    let marshal_group = |group_idx: &usize, fields: &Vec<FieldInfo>| {
        let field_idents: Vec<_> = fields.iter().map(|field| &field.ident).collect();
        let field_chunks = fields
            .chunks(7)
            .enumerate()
            .map(|(chunk_idx, chunk_fields)| {
                let chunk_start = chunk_idx * 7;
                let mask_bits = chunk_fields.iter().enumerate().map(|(bit, _)| {
                    let field_idx = chunk_start + bit;
                    quote! {
                        if field_dirty[#field_idx] {
                            field_mask |= 1 << #bit;
                        }
                    }
                });
                let payloads = chunk_fields.iter().enumerate().map(|(bit, field)| {
                    let field_idx = chunk_start + bit;
                    let ident = &field.ident;
                    quote! {
                        if field_dirty[#field_idx] {
                            self.#ident.marshal(wb);
                        }
                    }
                });
                let later_indices = (chunk_start + chunk_fields.len())..fields.len();
                let is_first = chunk_idx == 0;
                quote! {
                    {
                        let mut field_mask = 0u8;
                        #(#mask_bits)*
                        let later_dirty = false #(|| field_dirty[#later_indices])*;
                        if later_dirty {
                            field_mask |= 0x80;
                        }
                        if #is_first || field_mask != 0 {
                            wb.write_u8(field_mask);
                            #(#payloads)*
                        }
                    }
                }
            });

        quote! {
            if descriptor_dirty[#group_idx] {
                let baseline = mc.group_baselines.map_or(mc.baseline_seq, |baselines| {
                    baselines.baseline_for(
                        ::nw_network::hub::GroupIndex::new(#group_idx),
                        mc.baseline_seq,
                    )
                });
                let field_dirty = [#(
                    ::nw_network::ReplicatedFieldHandlerBase::is_dirty(
                        &self.#field_idents,
                        baseline,
                    )
                ),*];
                // The continuation bit (bit 7 of each chunk mask) means
                // "more dirty fields follow in a subsequent chunk". Live's
                // marshaler only sets it when dirty fields exist beyond the
                // current chunk window, not merely when more schema fields
                // exist.
                #(#field_chunks)*
            }
        }
    };
    let marshal_chunks = (0..=(max_descriptor / 8)).map(|chunk_idx| {
        let chunk_start = chunk_idx * 8;
        let chunk_end = chunk_start + 8;
        let descriptor_bits = groups
            .keys()
            .filter(|group_idx| **group_idx >= chunk_start && **group_idx < chunk_end)
            .map(|group_idx| {
                let bit = 1u8 << (group_idx % 8);
                quote! {
                    if descriptor_dirty[#group_idx] {
                        descriptor_mask |= #bit;
                    }
                }
            });
        let groups_in_chunk = groups
            .iter()
            .filter(|(group_idx, _)| **group_idx >= chunk_start && **group_idx < chunk_end)
            .map(|(group_idx, fields)| marshal_group(group_idx, fields));
        quote! {
            let mut descriptor_mask = 0u8;
            #(#descriptor_bits)*
            wb.write_u8(descriptor_mask);
            #(#groups_in_chunk)*
        }
    });

    let dirty_groups = groups.iter().map(|(group_idx, fields)| {
        let field_idents: Vec<_> = fields.iter().map(|field| &field.ident).collect();
        quote! {
            if mc
                .filter_target
                .map(|target| {
                    ::nw_network::hub::Fragment::should_send_to_client_group(
                        self,
                        target,
                        ::nw_network::hub::GroupIndex::new(#group_idx),
                    )
                })
                .unwrap_or(true)
            {
                let baseline = mc.group_baselines.map_or(mc.baseline_seq, |baselines| {
                    baselines.baseline_for(
                        ::nw_network::hub::GroupIndex::new(#group_idx),
                        mc.baseline_seq,
                    )
                });
                descriptor_dirty[#group_idx] = false #(
                    || ::nw_network::ReplicatedFieldHandlerBase::is_dirty(
                        &self.#field_idents,
                        baseline,
                    )
                )*;
            }
        }
    });

    let reset_fields = groups.values().flat_map(|fields| {
        fields.iter().map(|field| {
            let ident = &field.ident;
            quote! {
                ::nw_network::ReplicatedFieldHandlerBase::reset_has_new_network_data(
                    &mut self.#ident,
                );
            }
        })
    });

    let merge_fields = groups.values().flat_map(|fields| {
        fields.iter().map(|field| {
            let ident = &field.ident;
            quote! {
                outcome.detected_new_data_in_last_merge |=
                    ::nw_network::ReplicatedFieldHandlerBase::merge_and_update_sequence(
                        &mut merged_state.#ident,
                        &self.#ident,
                        &mut new_state.#ident,
                        seq,
                        inherit_previous_network_data_status,
                    );
                outcome.last_modified = outcome
                    .last_modified
                    .max(::nw_network::ReplicatedFieldHandlerBase::last_modified(
                        &merged_state.#ident,
                    ));
                outcome.has_new_network_data |=
                    ::nw_network::ReplicatedFieldHandlerBase::has_new_network_data(
                        &merged_state.#ident,
                    );
            }
        })
    });

    let marshal_metadata_fields = groups.values().flat_map(|fields| {
        fields.iter().map(|field| {
            let ident = &field.ident;
            quote! {
                ::nw_network::ReplicatedFieldHandlerBase::last_modified(&self.#ident).marshal(wb);
            }
        })
    });

    let unmarshal_metadata_fields = groups.values().flat_map(|fields| {
        fields.iter().map(|field| {
            let ident = &field.ident;
            quote! {
                let sequence = ::nw_network::hub::SequenceNumber::unmarshal(rb)?;
                ::nw_network::ReplicatedFieldHandlerBase::set_last_modified(
                    &mut self.#ident,
                    sequence,
                );
            }
        })
    });

    quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            fn __replicated_state_unmarshal_fields(
                &mut self,
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<(), ::nw_network::serialize::error::MarshalerError> {
                #(#unmarshal_chunks)*
                Ok(())
            }

            fn __replicated_state_marshal_fields(
                &self,
                mc: &::nw_network::hub::MarshalContext<'_>,
                wb: &mut ::nw_network::serialize::buffer::WriteBuffer,
            ) -> bool {
                use ::nw_network::serialize::marshaler::Marshaler as _;

                let mut descriptor_dirty = [false; #descriptor_count];
                #(#dirty_groups)*
                if !descriptor_dirty.iter().any(|dirty| *dirty) {
                    return false;
                }
                #(#marshal_chunks)*
                true
            }

            fn __replicated_state_reset_has_new_network_data(&mut self) {
                #(#reset_fields)*
            }

            fn __replicated_state_merge_fields(
                &self,
                new_state: &mut Self,
                merged_state: &mut Self,
                seq: ::nw_network::hub::SequenceNumber,
                inherit_previous_network_data_status: bool,
                outcome: &mut ::nw_network::hub::ReplicatedMergeOutcome,
            ) {
                #(#merge_fields)*
            }

            fn __replicated_state_marshal_field_metadata(
                &self,
                wb: &mut ::nw_network::serialize::buffer::WriteBuffer,
            ) -> bool {
                use ::nw_network::serialize::marshaler::Marshaler as _;
                #(#marshal_metadata_fields)*
                true
            }

            fn __replicated_state_unmarshal_field_metadata(
                &mut self,
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<bool, ::nw_network::serialize::error::MarshalerError> {
                use ::nw_network::serialize::marshaler::Marshaler as _;
                #(#unmarshal_metadata_fields)*
                Ok(true)
            }
        }
    }
}

fn fragment_category_expr(
    category: Option<&str>,
    span: proc_macro2::Span,
) -> syn::Result<TokenStream> {
    let Some(name) = category else {
        return Ok(quote! { ::nw_network::hub::FragmentCategory::Uncategorized });
    };
    Ok(match name {
        "uncategorized" => quote! { ::nw_network::hub::FragmentCategory::Uncategorized },
        "player_character" => quote! { ::nw_network::hub::FragmentCategory::PlayerCharacter },
        "non_player_character" | "npc" => {
            quote! { ::nw_network::hub::FragmentCategory::NonPlayerCharacter }
        }
        "important_non_player_character" | "important_npc" => {
            quote! { ::nw_network::hub::FragmentCategory::ImportantNonPlayerCharacter }
        }
        "spell" => quote! { ::nw_network::hub::FragmentCategory::Spell },
        "projectile" => quote! { ::nw_network::hub::FragmentCategory::Projectile },
        "buildable" => quote! { ::nw_network::hub::FragmentCategory::Buildable },
        other => {
            return Err(syn::Error::new(
                span,
                format!("unsupported replicated_state category `{other}`"),
            ));
        }
    })
}

fn expand_fragment(
    ident: &Ident,
    base: &BaseField,
    groups: &BTreeMap<usize, Vec<FieldInfo>>,
    opts: &Opts,
    impl_generics: &syn::ImplGenerics<'_>,
    ty_generics: &syn::TypeGenerics<'_>,
    where_clause: Option<&syn::WhereClause>,
) -> syn::Result<TokenStream> {
    let base_ident = &base.ident;
    let n_groups = groups.keys().copied().max().map_or(1, |max| max + 1);

    let category = fragment_category_expr(opts.category.as_deref(), ident.span())?;

    let calculate_default_bits_groups = groups.iter().map(|(group_idx, fields)| {
        let field_infos = fields.iter().map(|field| {
            let ident = &field.ident;
            let name = &field.name;
            quote! {
                ::nw_network::hub::ReplicatedFieldInfo {
                    name: #name,
                    handler: &self.#ident,
                    is_filter_group: false,
                }
            }
        });
        quote! {
            {
                let fields = [#(#field_infos),*];
                hub.calculate_default_bits(
                    ::nw_network::hub::GroupIndex::new(#group_idx),
                    &fields,
                    mc.baseline_seq,
                );
            }
        }
    });
    let calculate_default_bits_groups_for_metadata = groups.iter().map(|(group_idx, fields)| {
        let field_infos = fields.iter().map(|field| {
            let ident = &field.ident;
            let name = &field.name;
            quote! {
                ::nw_network::hub::ReplicatedFieldInfo {
                    name: #name,
                    handler: &self.#ident,
                    is_filter_group: false,
                }
            }
        });
        quote! {
            {
                let fields = [#(#field_infos),*];
                hub.calculate_default_bits(
                    ::nw_network::hub::GroupIndex::new(#group_idx),
                    &fields,
                    ::nw_network::hub::SequenceNumber::Invalid,
                );
            }
        }
    });
    let apply_default_bits_groups = groups.iter().map(|(group_idx, fields)| {
        let field_infos = fields.iter().map(|field| {
            let ident = &field.ident;
            let name = &field.name;
            quote! {
                ::nw_network::hub::ReplicatedFieldInfoMut {
                    name: #name,
                    handler: &mut self.#ident,
                    is_filter_group: false,
                }
            }
        });
        quote! {
            {
                let mut fields = [#(#field_infos),*];
                let hub = &self.#base_ident;
                hub.apply_default_bits(
                    ::nw_network::hub::GroupIndex::new(#group_idx),
                    &mut fields,
                );
            }
        }
    });

    let world_position_impl = match &opts.world_position {
        Some(method_name) if method_name.ends_with("()") => {
            let method_name = method_name.trim_end_matches("()");
            let method = Ident::new(method_name, ident.span());
            quote! {
                fn has_world_position(&self) -> bool {
                    true
                }

                fn world_position(&self) -> ::core::option::Option<::glam::Vec3> {
                    self.#method()
                }
            }
        }
        Some(field_name) => {
            let pos = Ident::new(field_name, ident.span());
            quote! {
                fn has_world_position(&self) -> bool {
                    true
                }

                fn world_position(&self) -> ::core::option::Option<::glam::Vec3> {
                    self.#pos.value.as_ref().map(|anchor| {
                        ::glam::Vec3::new(anchor.x, anchor.y, anchor.height)
                    })
                }
            }
        }
        None => quote! {},
    };

    Ok(quote! {
        impl #impl_generics ::nw_network::hub::DynFragment
        for #ident #ty_generics #where_clause
        {
            fn base(&self) -> &::nw_network::hub::FragmentBase {
                self.#base_ident.base()
            }

            fn base_mut(&mut self) -> &mut ::nw_network::hub::FragmentBase {
                self.#base_ident.base_mut()
            }

            fn marshal_contents(
                &self,
                wb: &mut ::nw_network::serialize::buffer::WriteBuffer,
            ) -> bool {
                self.marshal_contents_with(&::nw_network::hub::MarshalContext::default(), wb)
            }

            fn marshal_contents_with(
                &self,
                mc: &::nw_network::hub::MarshalContext<'_>,
                wb: &mut ::nw_network::serialize::buffer::WriteBuffer,
            ) -> bool {
                self.__replicated_state_marshal_fields(mc, wb)
            }

            fn unmarshal_contents(
                &mut self,
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<bool, ::nw_network::serialize::error::MarshalerError> {
                self.__replicated_state_unmarshal_fields(rb)?;
                Ok(true)
            }

            fn marshal_attributes(
                &self,
                mc: &::nw_network::hub::MarshalContext<'_>,
                wb: &mut ::nw_network::serialize::buffer::WriteBuffer,
            ) -> bool {
                let mut hub = self.#base_ident.clone();
                hub.ensure_filter_groups(#n_groups);
                #(#calculate_default_bits_groups)*
                hub.marshal_filter_group_attributes(mc.baseline_seq, wb)
            }

            fn unmarshal_attributes(
                &mut self,
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<bool, ::nw_network::serialize::error::MarshalerError> {
                self.#base_ident.ensure_filter_groups(#n_groups);
                let read_any = self.#base_ident.unmarshal_filter_group_attributes(rb)?;
                #(#apply_default_bits_groups)*
                Ok(read_any)
            }

            fn marshal_field_metadata(
                &self,
                _mc: &::nw_network::hub::MarshalContext<'_>,
                wb: &mut ::nw_network::serialize::buffer::WriteBuffer,
            ) -> bool {
                let mut hub = self.#base_ident.clone();
                hub.ensure_filter_groups(#n_groups);
                #(#calculate_default_bits_groups_for_metadata)*
                self.__replicated_state_marshal_field_metadata(wb);
                hub.marshal_filter_group_attribute_metadata(wb);
                true
            }

            fn unmarshal_field_metadata(
                &mut self,
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<bool, ::nw_network::serialize::error::MarshalerError> {
                let read_fields = self.__replicated_state_unmarshal_field_metadata(rb)?;
                self.#base_ident.ensure_filter_groups(#n_groups);
                let read_attrs = self.#base_ident.unmarshal_filter_group_attribute_metadata(rb)?;
                Ok(read_fields || read_attrs)
            }
        }

        impl #impl_generics ::nw_network::hub::Fragment
        for #ident #ty_generics #where_clause
        {
            fn merge_and_update_sequence(
                &self,
                new_fragment: &mut dyn ::nw_network::hub::Fragment,
                seq: ::nw_network::hub::SequenceNumber,
                inherit_previous_network_data_status: bool,
            ) -> ::core::option::Option<::std::boxed::Box<dyn ::nw_network::hub::Fragment>> {
                debug_assert!(seq.is_valid(), "Merge-to sequence should never be invalid");
                let new_correlation_id = new_fragment.correlation_id();
                let new_state = new_fragment.downcast_mut::<Self>()?;
                let mut merged_state = Self::default();
                let mut outcome = ::nw_network::hub::ReplicatedMergeOutcome::default();

                merged_state.#base_ident.ensure_filter_groups(#n_groups);
                self.__replicated_state_merge_fields(
                    new_state,
                    &mut merged_state,
                    seq,
                    inherit_previous_network_data_status,
                    &mut outcome,
                );
                merged_state.#base_ident.merge_filter_group_attributes(
                    &self.#base_ident,
                    &mut new_state.#base_ident,
                    seq,
                    inherit_previous_network_data_status,
                    &mut outcome,
                );
                merged_state
                    .#base_ident
                    .finish_merge(seq, new_correlation_id, outcome);
                ::core::option::Option::Some(::std::boxed::Box::new(merged_state))
            }

            fn reset_has_new_network_data(&mut self) {
                self.__replicated_state_reset_has_new_network_data();
                self.#base_ident.reset_filter_group_attribute_network_data();
                self.#base_ident.reset_has_new_network_data();
            }

            fn set_has_new_network_data_on_initial_state(&mut self) {
                self.#base_ident.set_has_new_network_data_on_initial_state();
            }

            fn is_fully_merged_state(&self) -> bool {
                self.#base_ident.is_fully_merged_state()
            }

            fn has_new_network_data(&self) -> bool {
                self.#base_ident.has_new_network_data()
            }

            fn detected_new_data_in_last_merge(&self) -> bool {
                self.#base_ident.detected_new_data_in_last_merge()
            }

            fn update_sequence(&self) -> ::nw_network::hub::SequenceNumber {
                self.#base_ident.sequence()
            }

            fn is_fragment_dirty(&self, baseline: ::nw_network::hub::SequenceNumber) -> bool {
                baseline < self.#base_ident.last_modified()
            }

            fn category(&self) -> ::nw_network::hub::FragmentCategory {
                #category
            }

            #world_position_impl

            fn num_filter_groups(&self) -> usize {
                #n_groups
            }

            fn should_send_to_client_group(
                &self,
                target: ::nw_network::hub::ClientActorHash,
                group_idx: ::nw_network::hub::GroupIndex,
            ) -> bool {
                group_idx.get() < #n_groups
                    && self.#base_ident.should_send_to_client(target, group_idx)
            }

            fn create_new_instance(&self) -> ::core::option::Option<::std::boxed::Box<dyn ::nw_network::hub::Fragment>> {
                ::core::option::Option::Some(::std::boxed::Box::new(Self::default()))
            }
        }
    })
}
