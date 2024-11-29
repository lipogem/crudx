use std::{
    any::Any,
    ops::{IndexMut, Not},
};

use sqlx::{postgres::PgRow, PgExecutor, QueryBuilder, Row};

use crate::{anyhow, Executor, Filter, LimitExecutor, OrderExecutor, Other, Result};

use super::Model;

pub trait Postgres<'a, T> {
    /// bind a database connection
    /// # Example
    /// ```no_run
    /// use sqlx::PgPool;
    ///
    /// let pool = PgPool::connect("postgresql://postgres:123456@127.0.0.1:5432/school")
    ///     .await
    ///     .unwrap();
    /// ...
    /// model.bind(&pool)
    /// ...
    /// ```
    fn bind<E>(self, executor: E) -> impl Executor<'a, T>
    where
        E: PgExecutor<'a>;

    /// bind database connections and customize conversion functions
    /// # Example
    /// ```no_run
    /// model.bind_conv(
    ///     &pool,
    ///     |value, query| {
    ///         if let Some(p) = value.downcast_ref::<u32>() {
    ///             query.push_bind(*p as i64);
    ///             return Ok(format!("{:?}", p));
    ///         } else if let Some(p) = value.downcast_ref::<Option<String>>() {
    ///             query.push_bind(p);
    ///             return Ok(format!("{:?}", p));
    ///         }
    ///         sqlx_to_arg!(value, query, String, &str, f64, f32, i64, i32, i16, i8, bool)
    ///     },
    ///     |name, row, value| {
    ///         if let Some(p) = value.downcast_mut::<u32>() {
    ///             *p = row.try_get::<i64, _>(name)? as u32;
    ///             return Ok(());
    ///         } else if let Some(p) = value.downcast_mut::<Option<String>>() {
    ///             *p = row.try_get::<Option<String>, _>(name)?;
    ///             return Ok(());
    ///         }
    ///         sqlx_from_row!(name, row, value, String, f64, f32, i64, i32, i16, i8, bool)
    ///     },
    /// )
    /// ```
    fn bind_conv<E, P, R>(self, executor: E, to_arg: P, from_row: R) -> impl Executor<'a, T>
    where
        E: PgExecutor<'a>,
        P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Postgres>) -> Result<String> + Send,
        R: Fn(&str, &PgRow, &mut dyn Any) -> Result<()> + Send;
}

impl<'a, T> Postgres<'a, T> for Model<'a, T>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
{
    fn bind<E>(self, executor: E) -> impl Executor<'a, T>
    where
        E: PgExecutor<'a>,
    {
        PostgresModel {
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
        E: PgExecutor<'a>,
        P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Postgres>) -> Result<String> + Send,
        R: Fn(&str, &PgRow, &mut dyn Any) -> Result<()> + Send,
    {
        PostgresModel {
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

struct PostgresModel<'a, T, E, P, R>
where
    E: PgExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Postgres>) -> Result<String> + Send,
    R: Fn(&str, &PgRow, &mut dyn Any) -> Result<()> + Send,
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
impl<'a, T, E, P, R> Executor<'a, T> for PostgresModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: PgExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Postgres>) -> Result<String> + Send,
    R: Fn(&str, &PgRow, &mut dyn Any) -> Result<()> + Send,
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
impl<'a, T, E, P, R> OrderExecutor<'a, T> for PostgresModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: PgExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Postgres>) -> Result<String> + Send,
    R: Fn(&str, &PgRow, &mut dyn Any) -> Result<()> + Send,
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
impl<'a, T, E, P, R> LimitExecutor<'a, T> for PostgresModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: PgExecutor<'a>,
    P: Fn(&'a dyn Any, &mut QueryBuilder<'a, sqlx::Postgres>) -> Result<String> + Send,
    R: Fn(&str, &PgRow, &mut dyn Any) -> Result<()> + Send,
{
    async fn query(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<Vec<T>> {
        Ok(sqlx_query!(self, filter, other, true))
    }
}
