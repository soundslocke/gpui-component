use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Resolve the GPUI API exposed to the crate where a macro is expanded.
///
/// `gpui-kit` is preferred because it re-exports GPUI and is the only direct
/// dependency required by kit consumers. The `gpui-pre` package fallback
/// preserves standalone `gpui-component` consumers, including dependencies
/// that rename that package to `gpui` (the conventional name). The plain
/// `gpui` fallback covers building against a Zed checkout, where the package
/// keeps its upstream name rather than the `gpui-pre` release name.
pub(crate) fn gpui() -> syn::Result<TokenStream> {
    let kit_error = match crate_name("gpui-kit") {
        Ok(found) => return Ok(found_crate_path(found)),
        Err(error) => error,
    };
    let pre_error = match crate_name("gpui-pre") {
        Ok(found) => return Ok(found_crate_path(found)),
        Err(error) => error,
    };
    crate_name("gpui").map(found_crate_path).map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!(
                "IntoPlot requires a direct dependency on `gpui-kit`, `gpui-pre` or `gpui`: \
                 gpui-kit lookup failed: {kit_error}; gpui-pre lookup failed: {pre_error}; \
                 gpui lookup failed: {error}"
            ),
        )
    })
}

fn found_crate_path(found: FoundCrate) -> TokenStream {
    match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
    }
}
