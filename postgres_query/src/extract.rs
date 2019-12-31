//! Extract typed values from rows.

use postgres_types::FromSql;
use std::fmt::Display;
use thiserror::Error;
use tokio_postgres::{error::Error as SqlError, Row};

/// An error that can occur while extracting values from a row.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{msg}")]
    Custom { msg: String },

    /// An error occured within postgres itself.
    #[error("internal postgres error")]
    Sql(#[from] SqlError),
}

impl Error {
    /// Construct a new error message with a custom message.
    pub fn new<D>(msg: D) -> Error
    where
        D: Display,
    {
        Error::Custom {
            msg: msg.to_string(),
        }
    }
}

/// Extract values from a row.
///
/// May be derived for `struct`s using `#[derive(FromSqlRow)]`.
///
/// # Example
///
/// ```
/// # use postgres_query_macro::FromSqlRow;
/// # use postgres_types::Date;
/// #[derive(FromSqlRow)]
/// struct Person {
///     age: i32,
///     name: String,
///     birthday: Option<Date<String>>,
/// }
/// ```
pub trait FromSqlRow: Sized {
    /// Extract values from a single row.
    fn from_row(row: &Row) -> Result<Self, Error>;

    /// Extract values from multiple rows.
    ///
    /// Implementors of this trait may override this method to enable optimizations not possible in
    /// [`from_row`] by, for example, only looking up the indices of columns with a specific name
    /// once.
    ///
    /// [`from_row`]: #tymethod.from_row
    fn from_row_multi(rows: &[Row]) -> Result<Vec<Self>, Error> {
        rows.iter().map(Self::from_row).collect()
    }
}

macro_rules! impl_for_tuple {
    (($($elem:ident),+)) => {
        impl<$($elem),+> FromSqlRow for ($($elem,)+)
            where $($elem: for<'a> FromSql<'a> + std::fmt::Display),+
        {
            fn from_row(row: &Row) -> Result<Self, Error> {
                // TODO: check that the number of columns match

                let result = (
                    $(
                        row.try_get::<usize, $elem>(
                            impl_for_tuple!(@index $elem)
                        )?,
                    )+
                );

                Ok(result)
            }
        }
    };

    (@index A) => { 0 };
    (@index B) => { 1 };
    (@index C) => { 2 };
    (@index D) => { 3 };
    (@index E) => { 4 };
    (@index F) => { 5 };
    (@index G) => { 6 };
    (@index H) => { 7 };
}

impl_for_tuple!((A));
impl_for_tuple!((A, B));
impl_for_tuple!((A, B, C));
impl_for_tuple!((A, B, C, D));
impl_for_tuple!((A, B, C, D, E));
impl_for_tuple!((A, B, C, D, E, F));
impl_for_tuple!((A, B, C, D, E, F, G));
impl_for_tuple!((A, B, C, D, E, F, G, H));
