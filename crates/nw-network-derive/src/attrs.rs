use darling::{FromDeriveInput, FromField, FromVariant};
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
