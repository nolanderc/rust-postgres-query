//! Extract typed values from rows.

use postgres_types::FromSql;
use std::fmt::{Display, Write};
use std::iter;
use std::ops::Range;
use thiserror::Error;
use tokio_postgres::{error::Error as SqlError, row::RowIndex, Column};

/// An error that can occur while extracting values from a row.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{msg}")]
    Custom { msg: String },

    #[error("invalid number of columns, found {found} but expected {expected}")]
    ColumnCount { found: usize, expected: usize },

    #[error("failed to get column: `{index}` (columns were: {columns})")]
    SliceLookup { index: String, columns: String },

    #[error("failed to split on: `{split}` (columns were: {columns})")]
    InvalidSplit { split: String, columns: String },

    #[error(
        "failed to slice row on: `{start}..{end}` (len was: {len})", 
        start = range.start,
        end = range.end
    )]
    SliceIndex { range: Range<usize>, len: usize },

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

mod private {
    pub mod row {
        pub trait Sealed {}
    }
}

/// Anything that provides a row-like interface.
///
/// This trait is sealed and cannot be implemented for types outside of this crate.
pub trait Row: private::row::Sealed {
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

    /// Return a subslice of this row's columns.
    fn slice(&self, range: Range<usize>) -> Result<RowSlice<Self>, Error>
    where
        Self: Sized,
    {
        if range.end > self.len() {
            Err(Error::SliceIndex {
                range,
                len: self.len(),
            })
        } else {
            let slice = RowSlice { row: self, range };
            Ok(slice)
        }
    }
}

/// A contiguous subset of columns in a row.
pub struct RowSlice<'a, R>
where
    R: Row,
{
    row: &'a R,
    range: Range<usize>,
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
    /// Number of columns required to construct this type.
    ///
    /// IMPORTANT: if not set correctly, extractors which depend on this value may produce errors.
    const COLUMN_COUNT: usize;

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

impl private::row::Sealed for tokio_postgres::Row {}

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

impl<R> private::row::Sealed for RowSlice<'_, R> where R: Row {}

impl<R> Row for RowSlice<'_, R>
where
    R: Row,
{
    fn columns(&self) -> &[Column] {
        &self.row.columns()[self.range.clone()]
    }

    fn try_get<'a, I, T>(&'a self, index: I) -> Result<T, Error>
    where
        I: RowIndex + Display,
        T: FromSql<'a>,
    {
        if let Some(index) = index.__idx(self.columns()) {
            self.row.try_get(self.range.start + index)
        } else {
            Err(Error::SliceLookup {
                index: index.to_string(),
                columns: format_columns(self.columns()),
            })
        }
    }
}

impl<R> RowSlice<'_, R>
where
    R: Row,
{
    /// Return a subslice of this row's columns.
    ///
    /// This is an optimized version of `Row::slice` which reduces the number of
    /// pointer-indirections.
    pub fn slice(&self, range: Range<usize>) -> Result<RowSlice<R>, Error>
    where
        Self: Sized,
    {
        if range.end > self.range.end {
            Err(Error::SliceIndex {
                range,
                len: self.range.end,
            })
        } else {
            let slice = RowSlice {
                row: self.row,
                range,
            };
            Ok(slice)
        }
    }
}

/// Split a row's columns into multiple partitions based on some split-points.
///
/// # Split
///
/// Given a list of column labels, a split is made right before the first column with a matching
/// name following the previous split:
///
/// ```text
/// Labels:       a,    a,      c,  a
/// Indices:      0 1 2 3 4 5 6 7 8 9 10
/// Columns:      a b c a b a b c b a c
/// Splits:      |     |       |   |   
/// Partitions: + +---+ +-----+ +-+ +-+
/// Ranges:     [0..0, 0..3, 3..7, 7..9, 9..11]`
/// ```
///
/// The first partition always contains the leading columns (zero or more):
///
/// ```text
/// Labels:         b,  a
/// Indices:    0 1 2 3 4 5
/// Columns:    d a b c a b
/// Splits:        |   |
/// Partitions: +-+ +-+ +-+
/// Ranges:     [0..2, 2..4, 4..6]
/// ```
///
/// # Errors
///
/// Will return an error if the columns could not be split (ie. no column with a matching name was
/// found in the remaining columns).
pub fn split_columns_many<'a, S>(
    columns: &'a [Column],
    splits: &'a [S],
) -> impl Iterator<Item = Result<Range<usize>, Error>> + 'a
where
    S: AsRef<str>,
{
    let column_names = columns.iter().map(|col| col.name());
    partition_many(column_names, splits.iter()).map(move |split| match split {
        SplitResult::Range(range) => Ok(range),
        SplitResult::NotFound { split, start } => Err(Error::InvalidSplit {
            split,
            columns: format_columns(&columns[start..]),
        }),
    })
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum SplitResult {
    NotFound { split: String, start: usize },
    Range(Range<usize>),
}

fn partition_many<'a>(
    columns: impl Iterator<Item = impl AsRef<str> + 'a> + 'a,
    splits: impl Iterator<Item = impl AsRef<str> + 'a> + 'a,
) -> impl Iterator<Item = SplitResult> + 'a {
    let mut columns = columns.enumerate();
    let mut splits = splits;

    let mut previous_end = 0;

    iter::from_fn(move || -> Option<_> {
        if let Some(split) = splits.next() {
            let split = split.as_ref();
            if let Some((end, _)) = columns.find(|(_, name)| name.as_ref() == split) {
                let range = previous_end..end;
                previous_end = end;
                Some(SplitResult::Range(range))
            } else {
                Some(SplitResult::NotFound {
                    split: split.to_owned(),
                    start: previous_end,
                })
            }
        } else {
            let (last, _) = columns.by_ref().last()?;
            let len = last + 1;
            Some(SplitResult::Range(previous_end..len))
        }
    })
}

fn format_columns(columns: &[Column]) -> String {
    let mut total = String::with_capacity(16 * columns.len());
    for col in columns {
        if !total.is_empty() {
            total.push_str(", ");
        }
        write!(total, "`{}`", col.name()).unwrap();
    }
    total
}

macro_rules! impl_from_row_for_tuple {
    (($($elem:ident),+)) => {
        impl<$($elem),+> FromSqlRow for ($($elem,)+)
            where $($elem: for<'a> FromSql<'a> + std::fmt::Display),+
        {
            const COLUMN_COUNT: usize = impl_from_row_for_tuple!(@count ($($elem),*));

            fn from_row<R>(row: &R) -> Result<Self, Error>
            where R: Row {
                if row.len() != Self::COLUMN_COUNT {
                    Err(Error::ColumnCount {
                        expected: Self::COLUMN_COUNT,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn split_chars_fallible<'a>(
        columns: &'a str,
        splits: &'a str,
    ) -> impl Iterator<Item = SplitResult> + 'a {
        let cols = columns.chars().map(|ch| ch.to_string());
        let splits = splits.chars().map(|ch| ch.to_string());
        partition_many(cols, splits)
    }

    fn split_chars<'a>(
        columns: &'a str,
        splits: &'a str,
    ) -> impl Iterator<Item = Range<usize>> + 'a {
        let cols = columns.chars().map(|ch| ch.to_string());
        let splits = splits.chars().map(|ch| ch.to_string());
        partition_many(cols, splits).map(move |split| match split {
            SplitResult::Range(range) => range,
            SplitResult::NotFound { split, start } => panic!(
                "failed to split {:?} on {:?}",
                columns.chars().skip(start).collect::<String>(),
                split,
            ),
        })
    }

    #[test]
    fn split_columns_many_no_excess() {
        let partitions = split_chars("abcabdab", "aaa").collect::<Vec<_>>();
        assert_eq!(partitions, vec![0..0, 0..3, 3..6, 6..8,])
    }

    #[test]
    fn split_columns_many_leading_columns() {
        let partitions = split_chars("deabcabdab", "aaa").collect::<Vec<_>>();
        assert_eq!(partitions, vec![0..2, 2..5, 5..8, 8..10,])
    }

    #[test]
    fn split_columns_many_too_many_splits() {
        let partitions = split_chars_fallible("abcabc", "aaa").collect::<Vec<_>>();
        assert_eq!(
            partitions,
            vec![
                SplitResult::Range(0..0),
                SplitResult::Range(0..3),
                SplitResult::NotFound {
                    split: "a".to_owned(),
                    start: 3,
                }
            ]
        )
    }
}
