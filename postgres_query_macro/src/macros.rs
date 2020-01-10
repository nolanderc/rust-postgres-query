macro_rules! err {
    ($item:expr, $msg:expr) => {
        syn::Error::new(syn::spanned::Spanned::span(&$item), $msg)
    };
    ($item:expr, $msg:expr, $($tt:tt)*) => {
        syn::Error::new(syn::spanned::Spanned::span(&$item), format!($msg, $($tt)*))
    };
}

macro_rules! lib {
    () => {{
        quote! { postgres_query }
    }};
}

macro_rules! is_match {
    ($expr:expr, $pattern:pat) => {
        match $expr {
            $pattern => true,
            _ => false,
        }
    };
}
