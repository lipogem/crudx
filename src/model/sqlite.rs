use std::{
    any::Any,
    ops::{IndexMut, Not},
};

use futures_util::StreamExt;
use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, SqliteExecutor};

use crate::{anyhow, Executor, Filter, LimitExecutor, OrderExecutor, Other, Result};

use super::Model;

pub trait Sqlite<'a, T> {
    /// bind a database connection
    /// # Example
    /// ```no_run
    /// use sqlx::SqlitePool;
    ///
    /// let pool = SqlitePool::connect("./school.db").await.unwrap();
    /// ...
    /// model.bind(&pool)
    /// ...
    /// ```
    fn bind<E>(self, executor: E) -> impl Executor<'a, T>
    where
        E: SqliteExecutor<'a>;

    /// bind database connections and customize conversion functions
    /// # Example
    /// ```no_run
    /// model.bind_conv(
    ///     &pool,
    ///     |value, query| {
    ///         if let Some(p) = value.downcast_ref::<Option<String>>() {
    ///             if let Some(t) = p {
    ///                 query.push_bind(t);
    ///             } else {
    ///                 query.push_bind("");
    ///             }
    ///             return Ok(format!("{:?}", p));
    ///         }
    ///         sqlx_to_arg!(value, query, String, &str, f64, f32, i64, i32, i16, i8, bool)
    ///     },
    ///     |name, row, value| {
    ///         if let Some(p) = value.downcast_mut::<Option<String>>() {
    ///             *p = Some(row.try_get::<String, _>(name)?);
    ///             return Ok(());
    ///         }
    ///         sqlx_from_row!(name, row, value, String, f64, f32, i64, i32, i16, i8, bool)
    ///     },
    /// )
    /// ```
    fn bind_conv<E, P, R>(self, executor: E, to_arg: P, from_row: R) -> impl Executor<'a, T>
    where
        E: SqliteExecutor<'a>,
        P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Sqlite>) -> Result<String> + Send,
        R: Fn(&str, &SqliteRow, &mut dyn Any) -> Result<()> + Send;
}

impl<'a, T> Sqlite<'a, T> for Model<'a, T>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
{
    fn bind<E>(self, executor: E) -> impl Executor<'a, T>
    where
        E: SqliteExecutor<'a>,
    {
        SqliteModel {
            model: self,
            executor,
            to_arg: |value, query| {
                sqlx_to_arg!(value, query, String, &str, f64, f32, i64, i32, i16, i8, bool)
            },
            from_row: |name, row, value| {
                sqlx_from_row!(name, row, value, String, f64, f32, i64, i32, i16, i8, bool)
            },
            order: "",
            limit: &0,
            offset: &0,
        }
    }

    fn bind_conv<E, P, R>(self, executor: E, to_arg: P, from_row: R) -> impl Executor<'a, T>
    where
        E: SqliteExecutor<'a>,
        P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Sqlite>) -> Result<String> + Send,
        R: Fn(&str, &SqliteRow, &mut dyn Any) -> Result<()> + Send,
    {
        SqliteModel {
            model: self,
            executor,
            to_arg,
            from_row,
            order: "",
            limit: &0,
            offset: &0,
        }
    }
}

struct SqliteModel<'a, T, E, P, R>
where
    E: SqliteExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Sqlite>) -> Result<String> + Send,
    R: Fn(&str, &SqliteRow, &mut dyn Any) -> Result<()> + Send,
{
    model: Model<'a, T>,
    executor: E,
    to_arg: P,
    from_row: R,
    order: &'a str,
    limit: &'a i64,
    offset: &'a i64,
}

#[cfg_attr(feature = "async_trait", async_trait::async_trait)]
impl<'a, T, E, P, R> Executor<'a, T> for SqliteModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: SqliteExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Sqlite>) -> Result<String> + Send,
    R: Fn(&str, &SqliteRow, &mut dyn Any) -> Result<()> + Send,
{
    async fn insert_one(self, filter: Option<&'a Filter>) -> Result<u64> {
        Ok(sqlx_insert_one!(self, filter))
    }

    async fn insert(self, data: &'a [T]) -> Result<u64> {
        Ok(sqlx_insert!(self, data))
    }

    async fn update(self, filter: &'a Filter) -> Result<u64> {
        Ok(sqlx_update!(self, filter))
    }

    async fn delete(self, filter: &'a Filter) -> Result<u64> {
        Ok(sqlx_delete!(self, filter))
    }

    async fn count(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<i64> {
        Ok(sqlx_count!(self, filter, other))
    }

    fn order_by(mut self, order: &'a str) -> impl OrderExecutor<'a, T> {
        self.order = order;
        self
    }
}

#[cfg_attr(feature = "async_trait", async_trait::async_trait)]
impl<'a, T, E, P, R> OrderExecutor<'a, T> for SqliteModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: SqliteExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Sqlite>) -> Result<String> + Send,
    R: Fn(&str, &SqliteRow, &mut dyn Any) -> Result<()> + Send,
{
    async fn query_one(mut self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<T> {
        self.limit = &1;
        self.offset = &0;
        let mut res = sqlx_query!(self, filter, other, true);
        res.pop().ok_or(anyhow!("no data found"))
    }

    async fn query(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<Vec<T>> {
        Ok(sqlx_query!(self, filter, other, false))
    }

    fn limit(mut self, limit: &'a i64, offset: &'a i64) -> impl LimitExecutor<'a, T> {
        self.limit = limit;
        self.offset = offset;
        self
    }
}

#[cfg_attr(feature = "async_trait", async_trait::async_trait)]
impl<'a, T, E, P, R> LimitExecutor<'a, T> for SqliteModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: SqliteExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Sqlite>) -> Result<String> + Send,
    R: Fn(&str, &SqliteRow, &mut dyn Any) -> Result<()> + Send,
{
    async fn query(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<Vec<T>> {
        Ok(sqlx_query!(self, filter, other, true))
    }
}
