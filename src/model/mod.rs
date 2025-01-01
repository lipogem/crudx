use std::{
    any::Any,
    collections::HashMap,
    ops::{IndexMut, Not},
};

#[macro_use]
mod macros;

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "postgres")]
pub use postgres::Postgres;

#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "mysql")]
pub use mysql::Mysql;

#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(feature = "sqlite")]
pub use sqlite::Sqlite;

#[cfg(feature = "mssql")]
mod mssql;
#[cfg(feature = "mssql")]
pub use mssql::Mssql;

pub struct Model<'a, T> {
    entity: &'a T,
    /// database table name
    pub table: String,
    /// database data table field names
    /// # Example
    /// ```no_run
    /// // skip name
    /// *model.fields.get_mut("name").unwrap() = "-";
    /// // select clazzName as name
    /// *model.fields.get_mut("name").unwrap() = "clazzName";
    /// ```
    pub fields: HashMap<&'a str, &'a str>,
}

impl<'a, T> Model<'a, T>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
{
    pub fn new(entity: &'a T) -> Model<'a, T> {
        let (name, fnames) = !entity;

        // table name camel hump to underline
        let mut table = String::new();
        for ch in name.chars() {
            if ch.is_uppercase() {
                if table.len() > 0 {
                    table.push('_');
                }
                table.push_str(&ch.to_lowercase().to_string());
            } else {
                table.push(ch);
            }
        }

        let fields = fnames
            .iter()
            .map(|&f| (f, ""))
            .collect::<HashMap<&'a str, &'a str>>();

        Model {
            entity,
            table,
            fields,
        }
    }
}
