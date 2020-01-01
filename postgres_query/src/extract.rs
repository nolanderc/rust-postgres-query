//! Extract typed values from rows.

use postgres_types::FromSql;
use std::fmt::Display;
use thiserror::Error;
use tokio_postgres::{error::Error as SqlError, row::RowIndex, Column};

/// An error that can occur while extracting values from a row.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{msg}")]
    Custom { msg: String },

    #[error("invalid number of columns, found {found} but expected {expected}")]
    ColumnCount { found: usize, expected: usize },

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

/// Anything that provides a row-like interface.
pub trait Row {
    /// Return the name and type of each column.
    fn columns(&self) -> &[Column];

    /// Attempt to get a cell in the row by the column name or index.
    fn try_get<'a, I, T>(&'a self, index: I) -> Result<T, Error>
    where
        I: RowIndex + Display,
        T: FromSql<'a>;

    /// The number of values (columns) in the row.
    fn len(&self) -> usize {
        self.columns().len()
    }

    /// `true` if the value did not contain any values, `false` otherwise.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Attempt to get a cell in the row by the column name or index.
    ///
    /// # Panics
    ///
    /// - If no cell was found with the given index.
    fn get<'a, I, T>(&'a self, index: I) -> T
    where
        I: RowIndex + Display,
        T: FromSql<'a>,
    {
        match self.try_get::<I, T>(index) {
            Ok(value) => value,
            Err(err) => panic!("failed to retrieve column: {}", err),
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
    fn from_row<R>(row: &R) -> Result<Self, Error>
    where
        R: Row;

    /// Extract values from multiple rows.
    ///
    /// Implementors of this trait may override this method to enable optimizations not possible in
    /// [`from_row`] by, for example, only looking up the indices of columns with a specific name
    /// once.
    ///
    /// [`from_row`]: #tymethod.from_row
    fn from_row_multi<R>(rows: &[R]) -> Result<Vec<Self>, Error>
    where
        R: Row,
    {
        rows.iter().map(Self::from_row).collect()
    }
}

impl Row for tokio_postgres::Row {
    fn columns(&self) -> &[Column] {
        tokio_postgres::Row::columns(self)
    }

    fn try_get<'a, I, T>(&'a self, index: I) -> Result<T, Error>
    where
        I: RowIndex + Display,
        T: FromSql<'a>,
    {
        tokio_postgres::Row::try_get(self, index).map_err(Error::from)
    }

    fn len(&self) -> usize {
        tokio_postgres::Row::len(self)
    }
    fn is_empty(&self) -> bool {
        tokio_postgres::Row::is_empty(self)
    }
    fn get<'a, I, T>(&'a self, index: I) -> T
    where
        I: RowIndex + Display,
        T: FromSql<'a>,
    {
        tokio_postgres::Row::get(self, index)
    }
}

macro_rules! impl_from_row_for_tuple {
    (($($elem:ident),+)) => {
        impl<$($elem),+> FromSqlRow for ($($elem,)+)
            where $($elem: for<'a> FromSql<'a> + std::fmt::Display),+
        {
            fn from_row<R>(row: &R) -> Result<Self, Error>
            where R: Row {
                // TODO: check that the number of columns match
                const EXPECTED: usize = impl_from_row_for_tuple!(@count ($($elem),*));
                if row.len() != EXPECTED {
                    Err(Error::ColumnCount {
                        expected: EXPECTED,
                        found: row.len(),
                    })
                } else {
                    let result = (
                        $(
                            row.try_get::<usize, $elem>(
                                impl_from_row_for_tuple!(@index $elem)
                            )?,
                        )+
                    );

                    Ok(result)
                }
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

    (@count ()) => { 0 };
    (@count ($head:ident $(, $tail:ident)*)) => {{
        1 + impl_from_row_for_tuple!(@count ($($tail),*))
    }};
}

impl_from_row_for_tuple!((A));
impl_from_row_for_tuple!((A, B));
impl_from_row_for_tuple!((A, B, C));
impl_from_row_for_tuple!((A, B, C, D));
impl_from_row_for_tuple!((A, B, C, D, E));
impl_from_row_for_tuple!((A, B, C, D, E, F));
impl_from_row_for_tuple!((A, B, C, D, E, F, G));
impl_from_row_for_tuple!((A, B, C, D, E, F, G, H));
