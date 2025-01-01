use std::{
    any::Any,
    ops::{IndexMut, Not},
};

use futures_util::{AsyncRead, AsyncWrite, StreamExt};
use tiberius::{Client, Row, ToSql};

use crate::{anyhow, Executor, Filter, LimitExecutor, OrderExecutor, Other, Result};

use super::Model;

pub trait Mssql<'a, T> {
    /// bind a database connection
    /// # Example
    /// ```no_run
    /// use tiberius::{AuthMethod, Client, Config, EncryptionLevel};
    /// use tokio::net::TcpStream;
    /// use tokio_util::compat::TokioAsyncWriteCompatExt;
    ///
    /// let mut config = Config::new();
    /// config.host("127.0.0.1");
    /// config.port(1433);
    /// config.authentication(AuthMethod::sql_server("sa", "123456"));
    /// config.encryption(EncryptionLevel::NotSupported);
    /// config.database("school");
    /// let tcp = TcpStream::connect(config.get_addr()).await.unwrap();
    /// tcp.set_nodelay(true).unwrap();
    /// let mut client = Client::connect(config, tcp.compat_write()).await.unwrap();
    /// ...
    /// model.bind(&mut client)
    /// ...
    /// ```
    fn bind<E>(self, executor: &'a mut Client<E>) -> impl Executor<'a, T>
    where
        E: AsyncRead + AsyncWrite + Unpin + Send;

    /// bind database connections and customize conversion functions
    /// # Example
    /// ```no_run
    /// ...
    /// model.bind_conv(
    ///     &mut client,
    ///     |value, args| {
    ///         if let Some(p) = value.downcast_ref::<NaiveDateTime>() {
    ///             args.push(p);
    ///             return Ok(p.to_string());
    ///         }
    ///         mssql_to_arg!(value, args, String, &str, f64, f32, i64, i32, i16, u8, bool)
    ///     },
    ///     |name, row, value| {
    ///         if let Some(p) = value.downcast_mut::<NaiveDateTime>() {
    ///             let ndt: Option<NaiveDateTime> = row.try_get(name)?;
    ///             if let Some(t) = ndt {
    ///                 *p = t;
    ///             }
    ///             return Ok(());
    ///         }
    ///         mssql_from_row!(name, row, value, f64, f32, i64, i32, i16, u8, bool)
    ///     },
    /// )
    /// ```
    fn bind_conv<E, P, R>(
        self,
        executor: &'a mut Client<E>,
        to_arg: P,
        from_row: R,
    ) -> impl Executor<'a, T>
    where
        E: AsyncRead + AsyncWrite + Unpin + Send,
        P: Fn(&'a dyn Any, &mut Vec<&'a dyn ToSql>) -> Result<String> + Send,
        R: Fn(&str, &Row, &mut dyn Any) -> Result<()> + Send;
}

impl<'a, T> Mssql<'a, T> for Model<'a, T>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
{
    fn bind<E>(self, executor: &'a mut Client<E>) -> impl Executor<'a, T>
    where
        E: AsyncRead + AsyncWrite + Unpin + Send,
    {
        MssqlModel {
            model: self,
            executor,
            to_arg: |value, args| {
                mssql_to_arg!(value, args, String, &str, f64, f32, i64, i32, i16, u8, bool)
            },
            from_row: |name, row, value| {
                mssql_from_row!(name, row, value, f64, f32, i64, i32, i16, u8, bool)
            },
            order: "",
            limit: &0,
            offset: &0,
        }
    }

    fn bind_conv<E, P, R>(
        self,
        executor: &'a mut Client<E>,
        to_arg: P,
        from_row: R,
    ) -> impl Executor<'a, T>
    where
        E: AsyncRead + AsyncWrite + Unpin + Send,
        P: Fn(&'a dyn Any, &mut Vec<&'a dyn ToSql>) -> Result<String> + Send,
        R: Fn(&str, &Row, &mut dyn Any) -> Result<()> + Send,
    {
        MssqlModel {
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

struct MssqlModel<'a, T, E, P, R>
where
    E: AsyncRead + AsyncWrite + Unpin + Send,
    P: Fn(&'a dyn Any, &mut Vec<&'a dyn ToSql>) -> Result<String> + Send,
    R: Fn(&str, &Row, &mut dyn Any) -> Result<()> + Send,
{
    model: Model<'a, T>,
    executor: &'a mut Client<E>,
    to_arg: P,
    from_row: R,
    order: &'a str,
    limit: &'a i64,
    offset: &'a i64,
}

#[cfg_attr(feature = "async_trait", async_trait::async_trait)]
impl<'a, T, E, P, R> Executor<'a, T> for MssqlModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: AsyncRead + AsyncWrite + Unpin + Send,
    P: Fn(&'a dyn Any, &mut Vec<&'a dyn ToSql>) -> Result<String> + Send,
    R: Fn(&str, &Row, &mut dyn Any) -> Result<()> + Send,
{
    async fn insert_one(self, filter: Option<&'a Filter>) -> Result<u64> {
        let (_, fnames) = !self.model.entity;

        let mut query = "insert into ".to_string();
        query.push_str(&self.model.table);

        //fields section
        let mut sep = false;
        query.push('(');
        for fd in fnames {
            if let Some(&co) = self.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            query.push(',');
                        }
                        query.push_str(co);
                        sep = true;
                    }
                } else {
                    if sep {
                        query.push(',');
                    }
                    query.push_str(fd);
                    sep = true;
                }
            }
        }
        query.push(')');

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = " ".to_string();

        //values section
        sep = false;
        if filter.is_some() {
            query.push_str(" select ");
        } else {
            query.push_str(" values (");
        }
        for (ix, fd) in fnames.iter().enumerate() {
            if let Some(&co) = self.model.fields.get(fd) {
                if co == "-" {
                    continue;
                }
            }
            if sep {
                query.push(',');
            }
            args.push_str(&(self.to_arg)(&self.model.entity[ix], &mut params)?);
            args.push(' ');
            query.push_str("@P");
            query.push_str(&params.len().to_string());
            sep = true;
        }

        //where statement section
        if let Some(flt) = filter {
            query.push_str(" where ");

            let mut idx = 0;
            let mut quo = false;
            for ch in flt.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= flt.args.len() {
                        return Err(anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&(self.to_arg)(&*flt.args[idx], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    idx += 1;
                } else {
                    query.push(ch);
                }
            }
            debug_assert!(!quo, "where statement quotation mark not closed error");

            if idx != flt.args.len() {
                return Err(anyhow!(
                    "the quantity of ? is not equal to the number of parameters"
                ));
            }
        } else {
            query.push(')');
        }

        //execute sql statements
        let res = match self.executor.execute(&query, &params).await {
            Ok(res) => res.total(),
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };

        Ok(res)
    }

    async fn insert(self, data: &'a [T]) -> Result<u64> {
        let (_, fnames) = !self.model.entity;

        let mut query = "insert into ".to_string();
        query.push_str(&self.model.table);

        //fields section
        let mut sep = false;
        query.push('(');
        for fd in fnames {
            if let Some(&co) = self.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            query.push(',');
                        }
                        query.push_str(co);
                        sep = true;
                    }
                } else {
                    if sep {
                        query.push(',');
                    }
                    query.push_str(fd);
                    sep = true;
                }
            }
        }
        query.push_str(")  values ");

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = String::new();

        //values section
        sep = false;
        for row in data {
            if sep {
                query.push(',');
                args.push(' ');
            }
            query.push('(');
            args.push_str("[ ");
            let mut sp = false;
            for (ix, fd) in fnames.iter().enumerate() {
                if let Some(&co) = self.model.fields.get(fd) {
                    if co == "-" {
                        continue;
                    }
                }
                if sp {
                    query.push(',');
                }
                args.push_str(&(self.to_arg)(&row[ix], &mut params)?);
                args.push(' ');
                query.push_str("@P");
                query.push_str(&params.len().to_string());
                sp = true;
            }
            query.push(')');
            args.push(']');
            sep = true;
        }

        //execute sql statements
        let res = match self.executor.execute(&query, &params).await {
            Ok(res) => res.total(),
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };

        Ok(res)
    }

    async fn update(self, filter: &'a Filter) -> Result<u64> {
        let (_, fnames) = !self.model.entity;

        let mut query = "update ".to_string();
        query.push_str(&self.model.table);
        query.push_str(" set ");

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = " ".to_string();

        //update statement section
        let mut sep = false;
        for (ix, fd) in fnames.iter().enumerate() {
            if let Some(&co) = self.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            query.push(',');
                        }
                        query.push_str(co);
                        query.push('=');
                        args.push_str(&(self.to_arg)(&self.model.entity[ix], &mut params)?);
                        args.push(' ');
                        query.push_str("@P");
                        query.push_str(&params.len().to_string());
                        sep = true;
                    }
                } else {
                    if sep {
                        query.push(',');
                    }
                    query.push_str(fd);
                    query.push('=');
                    args.push_str(&(self.to_arg)(&self.model.entity[ix], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    sep = true;
                }
            }
        }

        let mut idx = 0;

        //where statement section
        if filter.expr.len() > 0 {
            query.push_str(" where ");

            let mut quo = false;
            for ch in filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= filter.args.len() {
                        return Err(anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    idx += 1;
                } else {
                    query.push(ch);
                }
            }
            debug_assert!(!quo, "where statement quotation mark not closed error");
        }

        if idx != filter.args.len() {
            return Err(anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //execute sql statements
        let res = match self.executor.execute(&query, &params).await {
            Ok(res) => res.total(),
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };

        Ok(res)
    }

    async fn delete(self, filter: &'a Filter) -> Result<u64> {
        //from statement section
        let mut query = "delete from ".to_string();
        query.push_str(&self.model.table);

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = " ".to_string();
        let mut idx = 0;

        //where statement section
        if filter.expr.len() > 0 {
            query.push_str(" where ");

            let mut quo = false;
            for ch in filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= filter.args.len() {
                        return Err(anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    idx += 1;
                } else {
                    query.push(ch);
                }
            }
            debug_assert!(!quo, "where statement quotation mark not closed error");
        }

        if idx != filter.args.len() {
            return Err(anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //execute sql statements
        let res = match self.executor.execute(&query, &params).await {
            Ok(res) => res.total(),
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };

        Ok(res)
    }

    async fn count(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<i64> {
        //determine if there is group by
        let mut query = if matches!(&other,Some(ot) if ot.group_by.len() > 0) {
            "select count(*) from (select 1 as n".to_string()
        } else {
            "select count(*)".to_string()
        };

        //from statement section
        query.push_str(" from ");
        query.push_str(&self.model.table);

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = " ".to_string();
        let mut idx = 0;
        let mut quo = false;

        //join on statement section
        if let Some(ot) = &other {
            if ot.join_on.len() > 0 {
                query.push(' ');

                for ch in ot.join_on.chars() {
                    if ch == '\'' {
                        quo = !quo;
                    }
                    if ch == '?' && !quo {
                        if idx >= filter.args.len() {
                            return Err(anyhow!(" ? exceeds the number of args"));
                        }
                        args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                        args.push(' ');
                        query.push_str("@P");
                        query.push_str(&params.len().to_string());
                        idx += 1;
                    } else {
                        query.push(ch);
                    }
                }
            }
        }
        debug_assert!(!quo, "join on statement quotation mark not closed error");

        //where statement section
        if filter.expr.len() > 0 {
            query.push_str(" where ");

            for ch in filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= filter.args.len() {
                        return Err(anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    idx += 1;
                } else {
                    query.push(ch);
                }
            }
        }
        debug_assert!(!quo, "where statement quotation mark not closed error");

        //group by statement section
        if let Some(ot) = &other {
            if ot.group_by.len() > 0 {
                query.push(' ');
                query.push_str(ot.group_by);

                //having statement section
                if ot.having.len() > 0 {
                    query.push(' ');

                    for ch in ot.having.chars() {
                        if ch == '\'' {
                            quo = !quo;
                        }
                        if ch == '?' && !quo {
                            if idx >= filter.args.len() {
                                return Err(anyhow!(" ? exceeds the number of args"));
                            }
                            args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                            args.push(' ');
                            query.push_str("@P");
                            query.push_str(&params.len().to_string());
                            idx += 1;
                        } else {
                            query.push(ch);
                        }
                    }
                }

                query.push_str(") sub");
            }
        }
        debug_assert!(!quo, "having statement quotation mark not closed error");

        if idx != filter.args.len() {
            return Err(anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //execute sql statements
        let stream = match self.executor.query(&query, &params).await {
            Ok(stream) => stream,
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };
        let row = match stream.into_row().await {
            Ok(row) => row,
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };

        let res: Option<i32> = match row {
            Some(row) => row.try_get(0)?,
            None => return Err(anyhow!("sql:`{}` args:[{}]  result is None", query, args)),
        };
        match res {
            Some(res) => Ok(res as i64),
            None => Err(anyhow!("sql:`{}` args:[{}]  result is None", query, args)),
        }
    }

    fn order_by(mut self, order: &'a str) -> impl OrderExecutor<'a, T> {
        self.order = order;
        self
    }
}

#[cfg_attr(feature = "async_trait", async_trait::async_trait)]
impl<'a, T, E, P, R> OrderExecutor<'a, T> for MssqlModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: AsyncRead + AsyncWrite + Unpin + Send,
    P: Fn(&'a dyn Any, &mut Vec<&'a dyn ToSql>) -> Result<String> + Send,
    R: Fn(&str, &Row, &mut dyn Any) -> Result<()> + Send,
{
    async fn query_one(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<T> {
        let (_, fnames) = !self.model.entity;

        let mut query = "select top 1 ".to_string();

        //select statement section
        let mut sep = false;
        for fd in fnames {
            if let Some(&co) = self.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            query.push(',');
                        }
                        query.push_str(co);
                        query.push_str(" as ");
                        query.push_str(fd);
                        sep = true;
                    }
                } else {
                    if sep {
                        query.push(',');
                    }
                    query.push_str(fd);
                    sep = true;
                }
            }
        }

        //from statement section
        query.push_str(" from ");
        query.push_str(&self.model.table);

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = " ".to_string();
        let mut idx = 0;
        let mut quo = false;

        //join on statement section
        if let Some(ot) = &other {
            if ot.join_on.len() > 0 {
                query.push(' ');

                for ch in ot.join_on.chars() {
                    if ch == '\'' {
                        quo = !quo;
                    }
                    if ch == '?' && !quo {
                        if idx >= filter.args.len() {
                            return Err(anyhow!(" ? exceeds the number of args"));
                        }
                        args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                        args.push(' ');
                        query.push_str("@P");
                        query.push_str(&params.len().to_string());
                        idx += 1;
                    } else {
                        query.push(ch);
                    }
                }
            }
        }
        debug_assert!(!quo, "join on statement quotation mark not closed error");

        //where statement section
        if filter.expr.len() > 0 {
            query.push_str(" where ");

            for ch in filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= filter.args.len() {
                        return Err(anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    idx += 1;
                } else {
                    query.push(ch);
                }
            }
        }
        debug_assert!(!quo, "where statement quotation mark not closed error");

        //group by statement section
        if let Some(ot) = &other {
            if ot.group_by.len() > 0 {
                query.push(' ');
                query.push_str(ot.group_by);

                //having statement section
                if ot.having.len() > 0 {
                    query.push(' ');

                    for ch in ot.having.chars() {
                        if ch == '\'' {
                            quo = !quo;
                        }
                        if ch == '?' && !quo {
                            if idx >= filter.args.len() {
                                return Err(anyhow!(" ? exceeds the number of args"));
                            }
                            args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                            args.push(' ');
                            query.push_str("@P");
                            query.push_str(&params.len().to_string());
                            idx += 1;
                        } else {
                            query.push(ch);
                        }
                    }
                }
            }
        }
        debug_assert!(!quo, "having statement quotation mark not closed error");

        if idx != filter.args.len() {
            return Err(anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //order by statement section
        if self.order.len() > 0 {
            query.push_str(" order by ");
            query.push_str(self.order);
        }

        //execute sql statements
        let stream = match self.executor.query(&query, &params).await {
            Ok(stream) => stream,
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };
        let row = match stream.into_row().await {
            Ok(row) => row,
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };
        let row = match row {
            Some(row) => row,
            None => return Err(anyhow!("sql:`{}` args:[{}]  result is None", query, args)),
        };

        //convert data rows to entities
        let mut res = self.model.entity.clone();
        for (ix, fd) in fnames.iter().enumerate() {
            if let Some(&co) = self.model.fields.get(fd) {
                if co == "-" {
                    continue;
                }
            }
            (self.from_row)(fd, &row, &mut res[ix])?;
        }

        Ok(res)
    }

    async fn query(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<Vec<T>> {
        let (_, fnames) = !self.model.entity;

        let mut query = "select ".to_string();

        //select statement section
        let mut sep = false;
        for fd in fnames {
            if let Some(&co) = self.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            query.push(',');
                        }
                        query.push_str(co);
                        query.push_str(" as ");
                        query.push_str(fd);
                        sep = true;
                    }
                } else {
                    if sep {
                        query.push(',');
                    }
                    query.push_str(fd);
                    sep = true;
                }
            }
        }

        //from statement section
        query.push_str(" from ");
        query.push_str(&self.model.table);

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = " ".to_string();
        let mut idx = 0;
        let mut quo = false;

        //join on statement section
        if let Some(ot) = &other {
            if ot.join_on.len() > 0 {
                query.push(' ');

                for ch in ot.join_on.chars() {
                    if ch == '\'' {
                        quo = !quo;
                    }
                    if ch == '?' && !quo {
                        if idx >= filter.args.len() {
                            return Err(anyhow!(" ? exceeds the number of args"));
                        }
                        args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                        args.push(' ');
                        query.push_str("@P");
                        query.push_str(&params.len().to_string());
                        idx += 1;
                    } else {
                        query.push(ch);
                    }
                }
            }
        }
        debug_assert!(!quo, "join on statement quotation mark not closed error");

        //where statement section
        if filter.expr.len() > 0 {
            query.push_str(" where ");

            for ch in filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= filter.args.len() {
                        return Err(anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    idx += 1;
                } else {
                    query.push(ch);
                }
            }
        }
        debug_assert!(!quo, "where statement quotation mark not closed error");

        //group by statement section
        if let Some(ot) = &other {
            if ot.group_by.len() > 0 {
                query.push(' ');
                query.push_str(ot.group_by);

                //having statement section
                if ot.having.len() > 0 {
                    query.push(' ');

                    for ch in ot.having.chars() {
                        if ch == '\'' {
                            quo = !quo;
                        }
                        if ch == '?' && !quo {
                            if idx >= filter.args.len() {
                                return Err(anyhow!(" ? exceeds the number of args"));
                            }
                            args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                            args.push(' ');
                            query.push_str("@P");
                            query.push_str(&params.len().to_string());
                            idx += 1;
                        } else {
                            query.push(ch);
                        }
                    }
                }
            }
        }
        debug_assert!(!quo, "having statement quotation mark not closed error");

        if idx != filter.args.len() {
            return Err(anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //order by statement section
        if self.order.len() > 0 {
            query.push_str(" order by ");
            query.push_str(self.order);
        }

        //query column section
        let fds = fnames
            .iter()
            .enumerate()
            .filter_map(|(ix, fd)| {
                if let Some(&co) = self.model.fields.get(fd) {
                    if co == "-" {
                        return None;
                    }
                }
                Some((ix, fd))
            })
            .collect::<Vec<_>>();

        //execute sql statements
        let mut res = Vec::new();
        let mut stream = match self.executor.query(&query, &params).await {
            Ok(stream) => stream,
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };
        while let Some(rst) = stream.next().await {
            match rst {
                Ok(item) => {
                    if let Some(row) = item.as_row() {
                        let mut entity = self.model.entity.clone();
                        for (ix, fd) in &fds {
                            (self.from_row)(fd, row, &mut entity[*ix])?;
                        }
                        res.push(entity);
                    }
                }
                Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
            }
        }

        Ok(res)
    }

    fn limit(mut self, limit: &'a i64, offset: &'a i64) -> impl LimitExecutor<'a, T> {
        self.limit = limit;
        self.offset = offset;
        self
    }
}

#[cfg_attr(feature = "async_trait", async_trait::async_trait)]
impl<'a, T, E, P, R> LimitExecutor<'a, T> for MssqlModel<'a, T, E, P, R>
where
    T: IndexMut<usize, Output = dyn Any> + Clone + Send + Sync,
    &'a T: 'a + Not<Output = (&'static str, &'static [&'static str])>,
    E: AsyncRead + AsyncWrite + Unpin + Send,
    P: Fn(&'a dyn Any, &mut Vec<&'a dyn ToSql>) -> Result<String> + Send,
    R: Fn(&str, &Row, &mut dyn Any) -> Result<()> + Send,
{
    async fn query(self, filter: &'a Filter, other: Option<Other<'a>>) -> Result<Vec<T>> {
        let (_, fnames) = !self.model.entity;

        let mut query = "select * from (select ".to_string();

        //select statement section
        for fd in fnames {
            if let Some(&co) = self.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        query.push_str(co);
                        query.push_str(" as ");
                        query.push_str(fd);
                        query.push(',');
                    }
                } else {
                    query.push_str(fd);
                    query.push(',');
                }
            }
        }

        query.push_str("row_number() over (order by ");
        //order by statement section
        if self.order.len() > 0 {
            query.push_str(self.order);
        } else {
            query.push_str("(select 1)");
        }
        query.push_str(") as _num");

        //from statement section
        query.push_str(" from ");
        query.push_str(&self.model.table);

        let mut params: Vec<&dyn ToSql> = Vec::new();
        let mut args = " ".to_string();
        let mut idx = 0;
        let mut quo = false;

        //join on statement section
        if let Some(ot) = &other {
            if ot.join_on.len() > 0 {
                query.push(' ');

                for ch in ot.join_on.chars() {
                    if ch == '\'' {
                        quo = !quo;
                    }
                    if ch == '?' && !quo {
                        if idx >= filter.args.len() {
                            return Err(anyhow!(" ? exceeds the number of args"));
                        }
                        args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                        args.push(' ');
                        query.push_str("@P");
                        query.push_str(&params.len().to_string());
                        idx += 1;
                    } else {
                        query.push(ch);
                    }
                }
            }
        }
        debug_assert!(!quo, "join on statement quotation mark not closed error");

        //where statement section
        if filter.expr.len() > 0 {
            query.push_str(" where ");

            for ch in filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= filter.args.len() {
                        return Err(anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                    args.push(' ');
                    query.push_str("@P");
                    query.push_str(&params.len().to_string());
                    idx += 1;
                } else {
                    query.push(ch);
                }
            }
        }
        debug_assert!(!quo, "where statement quotation mark not closed error");

        //group by statement section
        if let Some(ot) = &other {
            if ot.group_by.len() > 0 {
                query.push(' ');
                query.push_str(ot.group_by);

                //having statement section
                if ot.having.len() > 0 {
                    query.push(' ');

                    for ch in ot.having.chars() {
                        if ch == '\'' {
                            quo = !quo;
                        }
                        if ch == '?' && !quo {
                            if idx >= filter.args.len() {
                                return Err(anyhow!(" ? exceeds the number of args"));
                            }
                            args.push_str(&(self.to_arg)(&*filter.args[idx], &mut params)?);
                            args.push(' ');
                            query.push_str("@P");
                            query.push_str(&params.len().to_string());
                            idx += 1;
                        } else {
                            query.push(ch);
                        }
                    }
                }
            }
        }
        debug_assert!(!quo, "having statement quotation mark not closed error");

        if idx != filter.args.len() {
            return Err(anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //limit statement section
        query.push_str(") sub where _num between (1+");
        params.push(self.offset);
        args.push_str(&(self.offset.to_string() + " "));
        query.push_str("@P");
        query.push_str(&params.len().to_string());
        query.push_str(") and (");
        query.push_str("@P");
        query.push_str(&params.len().to_string());
        query.push('+');
        params.push(self.limit);
        args.push_str(&(self.limit.to_string() + " "));
        query.push_str("@P");
        query.push_str(&params.len().to_string());
        query.push(')');

        //query column section
        let fds = fnames
            .iter()
            .enumerate()
            .filter_map(|(ix, fd)| {
                if let Some(&co) = self.model.fields.get(fd) {
                    if co == "-" {
                        return None;
                    }
                }
                Some((ix, fd))
            })
            .collect::<Vec<_>>();

        //execute sql statements
        let mut res = Vec::new();
        let mut stream = match self.executor.query(&query, &params).await {
            Ok(stream) => stream,
            Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
        };
        while let Some(rst) = stream.next().await {
            match rst {
                Ok(item) => {
                    if let Some(row) = item.as_row() {
                        let mut entity = self.model.entity.clone();
                        for (ix, fd) in &fds {
                            (self.from_row)(fd, row, &mut entity[*ix])?;
                        }
                        res.push(entity);
                    }
                }
                Err(err) => return Err(anyhow!("sql:`{}` args:[{}]  {}", query, args, err)),
            }
        }

        Ok(res)
    }
}
