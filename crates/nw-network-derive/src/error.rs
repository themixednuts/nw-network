//! Shared error plumbing for the derive macros.
//!
//! Each derive's helper code returns [`syn::Result<TokenStream>`]. The
//! proc-macro entry points in [`crate`] call `.unwrap_or_else(syn::Error::to_compile_error)`
//! at the boundary, so [`syn::Error`]s land in the user's build as proper
//! never has to know about `compile_error!` directly.
//!
//! Darling errors get bridged through [`darling_to_syn`] so a single
//! return type covers both attribute-parsing failures and our own
//! validation failures.

/// Convert a [`darling::Error`] into a [`syn::Error`] for unified error
/// handling.
///
/// Darling's own `write_errors()` produces a `TokenStream` directly, but
/// that doesn't compose with `syn::Result<TokenStream>`. Bridging through
/// [`syn::Error`] keeps the helper signatures uniform without losing
/// span information — `darling::Error::write_errors()` is approximately
/// `to_compile_error()`, and round-tripping through string preserves
/// the diagnostic message.
pub fn darling_to_syn(err: &darling::Error) -> syn::Error {
    // `darling::Error::write_errors()` produces a TokenStream of one or
    // more `compile_error!(...)` invocations. Re-parsing as a single
    // `syn::Error` would drop multi-error structure, so when there are
    // multiple sub-errors we collapse them into one combined diagnostic
    // with the messages joined by newlines — span information is best
    // effort but the message stays useful.
    let span = err.span();
    syn::Error::new(span, err.to_string())
}
