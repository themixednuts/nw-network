use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Expr, ExprAssign, ExprLit, Lit, LitStr, Meta, Token};

struct Opts {
    type_id: Option<u128>,
    type_name: Option<LitStr>,
}

pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let opts = parse_opts(input)?;
    let ident = &input.ident;
    let type_id = opts
        .type_id
        .ok_or_else(|| syn::Error::new_spanned(input, "#[az_rtti(\"...\")] is required"))?;
    let type_name = opts
        .type_name
        .unwrap_or_else(|| LitStr::new(&ident.to_string(), ident.span()));
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::nw_network::types::AzRtti for #ident #ty_generics #where_clause {
            const TYPE_ID: ::uuid::Uuid = ::uuid::Uuid::from_u128(#type_id);
            const TYPE_NAME: &'static str = #type_name;
        }
    })
}

fn parse_opts(input: &DeriveInput) -> syn::Result<Opts> {
    let mut opts = Opts {
        type_id: None,
        type_name: None,
    };

    for attr in &input.attrs {
        if !attr.path().is_ident("az_rtti") {
            continue;
        }

        let Meta::List(list) = &attr.meta else {
            return Err(syn::Error::new_spanned(
                attr,
                "#[az_rtti(\"...\")] requires arguments",
            ));
        };
        let args = list.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;
        for (index, expr) in args.iter().enumerate() {
            parse_arg(&mut opts, expr, index)?;
        }
    }

    Ok(opts)
}

fn parse_arg(opts: &mut Opts, expr: &Expr, index: usize) -> syn::Result<()> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) if index == 0 => {
            opts.type_id = Some(parse_uuid_lit(value)?);
            Ok(())
        }
        Expr::Assign(assign) => parse_assignment(opts, assign),
        _ => Err(syn::Error::new_spanned(
            expr,
            "unsupported az_rtti argument",
        )),
    }
}

fn parse_assignment(opts: &mut Opts, assign: &ExprAssign) -> syn::Result<()> {
    let Expr::Path(path) = &*assign.left else {
        return Err(syn::Error::new_spanned(
            &assign.left,
            "az_rtti assignment key must be an identifier",
        ));
    };
    let Some(key) = path.path.get_ident() else {
        return Err(syn::Error::new_spanned(
            &assign.left,
            "az_rtti assignment key must be an identifier",
        ));
    };

    match key.to_string().as_str() {
        "uuid" => {
            opts.type_id = Some(match &*assign.right {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(value),
                    ..
                }) => parse_uuid_lit(value)?,
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "az_rtti uuid must be a string literal",
                    ));
                }
            });
            Ok(())
        }
        "name" => {
            let Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            }) = &*assign.right
            else {
                return Err(syn::Error::new_spanned(
                    &assign.right,
                    "az_rtti name must be a string literal",
                ));
            };
            opts.type_name = Some(value.clone());
            Ok(())
        }
        _ => Err(syn::Error::new_spanned(
            &assign.left,
            "unsupported az_rtti assignment",
        )),
    }
}

fn parse_uuid_lit(value: &LitStr) -> syn::Result<u128> {
    parse_uuid_to_u128(&value.value()).map_err(|message| syn::Error::new_spanned(value, message))
}

fn parse_uuid_to_u128(value: &str) -> Result<u128, String> {
    let cleaned = value
        .trim_matches(|ch| ch == '{' || ch == '}')
        .chars()
        .filter(|ch| *ch != '-')
        .collect::<String>();
    if cleaned.len() != 32 || !cleaned.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("invalid AZ RTTI UUID `{value}`"));
    }
    u128::from_str_radix(&cleaned, 16).map_err(|err| err.to_string())
}
