extern crate proc_macro;

mod attrs;

use crate::attrs::*;
use quote::*;
use std::collections::HashSet;
use syn::punctuated::*;
use syn::*;

#[proc_macro_derive(Query, attributes(query))]
pub fn define_query(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    let attrs = match Attrs::from_input(&input) {
        Ok(attrs) => attrs,
        Err(err) => return err.to_compile_error().into(),
    };

    let (sql, params) = match attrs.parse_sql_literal() {
        Ok(sql) => sql,
        Err(err) => return err.to_compile_error().into(),
    };

    let mut sorted_params = params.iter().collect::<Vec<_>>();
    sorted_params.sort_by_key(|(_, index)| *index);

    let sorted_params = sorted_params
        .into_iter()
        .map(|(ident, _)| ident)
        .collect::<Vec<_>>();

    match input.data {
        Data::Struct(DataStruct {
            fields,
            ..
        }) => {
            let fields = match fields {
                Fields::Named(fields) => fields.named,
                Fields::Unit => Default::default(),
                Fields::Unnamed(fields) => return Error::new_spanned(fields, "Cannot derive `Query` for tuple struct")
                    .to_compile_error().into()
            };

            let references_trait_object = fields
                .iter()
                .filter(|field| match &field.ty {
                    Type::Reference(reference) => match *reference.elem {
                        Type::TraitObject(_) => true,
                        _ => false,
                    },
                    _ => false,
                })
                .filter_map(|field| field.ident.as_ref())
                .collect::<HashSet<_>>();

            let param_borrow = sorted_params.iter().map(|ident| {
                if references_trait_object.contains(ident) {
                    None
                } else {
                    Some(quote!(&))
                }
            });

            let ident = input.ident;
            let param_count = params.len();

            let where_clause = input.generics.where_clause;
            let generic_params = input.generics.params.into_iter().collect::<Vec<_>>();
            let generic_tokens = generic_params
                .iter()
                .map(|param| match param {
                    GenericParam::Type(TypeParam { ident, .. }) => quote!(#ident),
                    GenericParam::Lifetime(LifetimeDef { lifetime, .. }) => quote!(#lifetime),
                    GenericParam::Const(ConstParam { ident, .. }) => quote!(#ident),
                })
                .collect::<Punctuated<_, Token![,]>>();

            let output = quote! {
                impl<'__query_params #(, #generic_params)*> ::postgres_query::Query<'__query_params> for #ident <#generic_tokens> #where_clause {
                    type Sql = &'static str;
                    type Params = [&'__query_params dyn ::postgres::types::ToSql; #param_count];

                    fn sql(&self) -> Self::Sql {
                        #sql
                    }

                    fn params(&'__query_params self) -> Self::Params {
                        [
                            #( #param_borrow self.#sorted_params, )*
                        ]
                    }
                }
            };

            output
        }

        Data::Enum(data) => Error::new_spanned(
            data.enum_token,
            "Cannot derive `Query` for enum",
        )
        .to_compile_error(),

        Data::Union(data) => Error::new_spanned(
            data.union_token,
            "Cannot derive `Query` for union",
        )
        .to_compile_error(),
    }.into()
}
