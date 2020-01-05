macro_rules! err {
    ($item:expr, $msg:literal) => {
        syn::Error::new(syn::spanned::Spanned::span(&$item), $msg)
    };
    ($item:expr, $msg:literal, $($tt:tt)*) => {
        syn::Error::new(syn::spanned::Spanned::span(&$item), format!($msg, $($tt)*))
    };
}

macro_rules! lib {
    () => {{
        quote! { postgres_query }
    }};
}

