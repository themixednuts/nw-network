use darling::{FromDeriveInput, FromField, FromVariant};
use syn::spanned::Spanned;
use syn::{Ident, Path};

/// Container-level attributes for the Marshaler derive
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(marshal), supports(struct_any, enum_any))]
pub struct MarshalerOpts {
    pub ident: Ident,
    pub generics: syn::Generics,
    pub data: darling::ast::Data<MarshalerVariant, MarshalerField>,
}

/// Field-level attributes
#[derive(Debug, FromField)]
#[darling(attributes(marshal))]
pub struct MarshalerField {
    pub ident: Option<Ident>,
    pub ty: syn::Type,

    /// Marshal as a different type: `#[marshal(as = "Vec<u8>")]`
    /// The field will be converted to/from this type during marshaling
    /// (writes via `<As as From<Field>>::from(self.field).marshal(wb)`,
    /// reads via `<Field as From<As>>::from(<As>::unmarshal(rb)?)`).
    ///
    /// `rename` is required because `as` is a reserved keyword in Rust;
    /// the field name has to be `r#as` while the attribute key the user
    /// writes is the bare `as`.
    #[darling(default, rename = "as")]
    pub r#as: Option<syn::Type>,

    /// Marshal with a [`Codec`](nw_network::serialize::Codec) policy:
    /// `#[marshal(codec = "ConversionMarshaler<u8, GridSides>")]`.
    ///
    /// Unlike `as`, this keeps the field's Rust type unchanged and uses the
    /// policy type only for encoding/decoding.
    #[darling(default)]
    pub codec: Option<syn::Type>,

    /// Custom marshaler function path: `#[marshal(with = "path::to::marshal_fn")]`
    #[darling(default)]
    pub with: Option<Path>,

    /// Custom unmarshaler function path: `#[marshal(unmarshal_with = "path::to::unmarshal_fn")]`
    #[darling(default)]
    pub unmarshal_with: Option<Path>,

    /// Skip this field during marshaling
    #[darling(default)]
    pub skip: bool,
}

/// Variant-level attributes for enums.
#[derive(Debug)]
pub struct MarshalerVariant {
    pub ident: Ident,
    pub fields: darling::ast::Fields<MarshalerField>,

    /// Explicit discriminant value for this variant from
    /// `#[marshal(discriminant = N)]`. When unset, the marshal/unmarshal
    /// codegen falls back to the variant's positional index.
    ///
    /// Parsed by hand below — darling 0.21's default `FromMeta` impls for
    /// primitive integer types don't compose cleanly with the rest of the
    /// `#[marshal(...)]` namespace here, so we walk the attribute syntax
    /// directly instead of relying on the derive.
    pub discriminant: Option<u64>,

    /// Catch-all variant for direct-repr enums:
    /// `#[marshal(unknown)] Unknown(u32)`.
    #[allow(dead_code)]
    pub unknown: bool,
}

impl FromVariant for MarshalerVariant {
    fn from_variant(variant: &syn::Variant) -> darling::Result<Self> {
        let ident = variant.ident.clone();
        let fields = darling::ast::Fields::try_from(&variant.fields)?;

        let mut discriminant: Option<u64> = match &variant.discriminant {
            Some((_, syn::Expr::Lit(expr))) => match &expr.lit {
                syn::Lit::Int(lit) => {
                    Some(lit.base10_parse::<u64>().map_err(darling::Error::from)?)
                }
                _ => {
                    return Err(darling::Error::custom(
                        "Marshaler enum discriminants must be integer literals",
                    ));
                }
            },
            Some(_) => {
                return Err(darling::Error::custom(
                    "Marshaler enum discriminants must be integer literals",
                ));
            }
            None => None,
        };
        let mut unknown = false;
        for attr in &variant.attrs {
            if !attr.path().is_ident("marshal") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("discriminant") {
                    let value = meta.value()?;
                    let lit: syn::LitInt = value.parse()?;
                    discriminant = Some(lit.base10_parse::<u64>()?);
                } else if meta.path.is_ident("unknown") {
                    unknown = true;
                }
                Ok(())
            })
            .map_err(darling::Error::from)?;
        }

        Ok(Self {
            ident,
            fields,
            discriminant,
            unknown,
        })
    }
}

impl MarshalerVariant {
    /// Resolve the wire discriminant: explicit attribute if supplied,
    /// otherwise the variant's positional index.
    pub fn discriminant_value(&self, default_index: u64) -> u64 {
        self.discriminant.unwrap_or(default_index)
    }
}

impl MarshalerOpts {
    pub fn validate(&self) -> syn::Result<()> {
        match &self.data {
            darling::ast::Data::Struct(fields) => validate_fields(fields.iter()),
            darling::ast::Data::Enum(variants) => {
                for variant in variants {
                    validate_fields(variant.fields.iter())?;
                }
                Ok(())
            }
        }
    }
}

fn validate_fields<'a>(fields: impl IntoIterator<Item = &'a MarshalerField>) -> syn::Result<()> {
    for field in fields {
        field.validate()?;
    }
    Ok(())
}

impl MarshalerField {
    fn validate(&self) -> syn::Result<()> {
        let mut modes = Vec::new();
        if let Some(ty) = &self.codec {
            modes.push(("codec", ty.span()));
        }
        if let Some(ty) = &self.r#as {
            modes.push(("as", ty.span()));
        }
        if let Some(path) = &self.with {
            modes.push(("with", path.span()));
        }
        if let Some(path) = &self.unmarshal_with {
            modes.push(("unmarshal_with", path.span()));
        }

        if self.skip && !modes.is_empty() {
            return Err(syn::Error::new(
                modes[0].1,
                "#[marshal(skip)] cannot be combined with marshal conversion attributes",
            ));
        }

        let has_codec = self.codec.is_some();
        let has_as = self.r#as.is_some();
        let has_custom = self.with.is_some() || self.unmarshal_with.is_some();
        let active_modes = usize::from(has_codec) + usize::from(has_as) + usize::from(has_custom);
        if active_modes > 1 {
            return Err(syn::Error::new(
                modes[1].1,
                "#[marshal(...)] accepts only one wire conversion mode: codec, as, or custom with/unmarshal_with",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use darling::FromDeriveInput;
    use syn::DeriveInput;

    use super::MarshalerOpts;

    fn parse(input: DeriveInput) -> syn::Result<MarshalerOpts> {
        let opts = MarshalerOpts::from_derive_input(&input)
            .map_err(|err| syn::Error::new(proc_macro2::Span::call_site(), err.to_string()))?;
        opts.validate()?;
        Ok(opts)
    }

    #[test]
    fn marshal_codec_and_as_conflict() {
        let err = parse(syn::parse_quote! {
            struct Value {
                #[marshal(codec = "Codec", as = "Wire")]
                field: u32,
            }
        })
        .unwrap_err();

        assert!(err.to_string().contains("only one wire conversion mode"));
    }

    #[test]
    fn marshal_custom_pair_is_one_mode() {
        parse(syn::parse_quote! {
            struct Value {
                #[marshal(with = "write_field", unmarshal_with = "read_field")]
                field: u32,
            }
        })
        .unwrap();
    }

    #[test]
    fn marshal_skip_and_conversion_conflict() {
        let err = parse(syn::parse_quote! {
            struct Value {
                #[marshal(skip, codec = "Codec")]
                field: u32,
            }
        })
        .unwrap_err();

        assert!(err.to_string().contains("skip"));
    }
}
