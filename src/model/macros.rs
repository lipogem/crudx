#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
#[macro_export]
macro_rules! sqlx_to_arg {
    ($value:ident, $query:ident, $($typ:ty),* $(,)?) => {{
        if let Some(p) = $value.downcast_ref::<Vec<u8>>() {
            $query.push_bind(p);
            Ok(format!("{:?}",p))
        } $(else if let Some(p) = $value.downcast_ref::<$typ>() {
            $query.push_bind(p);
            Ok(p.to_string())
        })* else {
            Err($crate::anyhow!("arguments after `{}` cannot be converted please customize the function to_arg",$query.sql()))
        }
    }};
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
#[macro_export]
macro_rules! sqlx_from_row {
    ($name:ident, $row:ident, $value:ident, $($typ:ty),* $(,)?) => {{
        if let Some(p) = $value.downcast_mut::<Vec<u8>>() {
            *p = sqlx::Row::try_get($row,$name)?;
        } $(else if let Some(p) = $value.downcast_mut::<$typ>() {
            *p = sqlx::Row::try_get($row,$name)?;
        })* else{
            return Err($crate::anyhow!("`{}` cannot be converted please customize the function from_row",$name));
        }
        Ok(())
    }};
}

#[cfg(feature = "mssql")]
#[macro_export]
macro_rules! mssql_to_arg {
    ($value:ident, $args:ident, $($typ:ty),* $(,)?) => {{
        if let Some(p) = $value.downcast_ref::<Vec<u8>>() {
            $args.push(p);
            Ok(format!("{:?}",p))
        } $(else if let Some(p) = $value.downcast_ref::<$typ>() {
            $args.push(p);
            Ok(p.to_string())
        })* else {
            Err($crate::anyhow!("arguments at index {} cannot be converted please customize the function to_arg",$args.len()))
        }
    }};
}

#[cfg(feature = "mssql")]
#[macro_export]
macro_rules! mssql_from_row {
    ($name:ident, $row:ident, $value:ident, $($typ:ty),* $(,)?) => {{
        if let Some(p) = $value.downcast_mut::<String>() {
            if let Some(t) = $row.try_get::<'_,&str,_>($name)? {
                *p = t.to_string();
            }
        } else if let Some(p) = $value.downcast_mut::<Vec<u8>>() {
            if let Some(t) = $row.try_get::<'_,&[u8],_>($name)? {
                *p = t.to_vec();
            }
        } $(else if let Some(p) = $value.downcast_mut::<$typ>() {
            if let Some(t) = $row.try_get($name)? {
                *p = t;
            }
        })* else{
            return Err($crate::anyhow!("`{}` cannot be converted please customize the function from_row",$name));
        }
        Ok(())
    }};
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
macro_rules! sqlx_insert_one {
    ($my:ident, $filter:ident) => {{
        let (_, fnames) = !$my.model.entity;

        let mut builder = QueryBuilder::new("insert into ");
        builder.push($my.model.table);

        //fields section
        let mut sep = false;
        builder.push("(");
        for fd in fnames {
            if let Some(&co) = $my.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            builder.push(",");
                        }
                        builder.push(co);
                        sep = true;
                    }
                } else {
                    if sep {
                        builder.push(",");
                    }
                    builder.push(fd);
                    sep = true;
                }
            }
        }
        builder.push(")");

        let mut args = " ".to_string();

        //values section
        sep = false;
        if $filter.is_some() {
            builder.push(" select ");
        } else {
            builder.push(" values (");
        }
        for (ix, fd) in fnames.iter().enumerate() {
            if let Some(&co) = $my.model.fields.get(fd) {
                if co == "-" {
                    continue;
                }
            }
            if sep {
                builder.push(",");
            }
            args.push_str(&($my.to_arg)(&$my.model.entity[ix], &mut builder)?);
            args.push(' ');
            sep = true;
        }

        //where statement section
        if let Some(flt) = $filter {
            builder.push(" where ");

            let mut idx = 0;
            let mut quo = false;
            for ch in flt.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= flt.args.len() {
                        return Err($crate::anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&($my.to_arg)(&*flt.args[idx], &mut builder)?);
                    args.push(' ');
                    idx += 1;
                } else {
                    builder.push(ch);
                }
            }
            debug_assert!(!quo, "where statement quotation mark not closed error");

            if idx != flt.args.len() {
                return Err($crate::anyhow!(
                    "the quantity of ? is not equal to the number of parameters"
                ));
            }
        } else {
            builder.push(")");
        }

        //execute sql statements
        let res = match builder.build().execute($my.executor).await {
            Ok(res) => res.rows_affected(),
            Err(err) => {
                return Err($crate::anyhow!(
                    "sql:`{}` args:[{}]  {}",
                    builder.sql(),
                    args,
                    err
                ))
            }
        };

        res
    }};
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
macro_rules! sqlx_insert {
    ($my:ident, $data:ident) => {{
        let (_, fnames) = !$my.model.entity;

        let mut builder = QueryBuilder::new("insert into ");
        builder.push($my.model.table);

        //fields section
        let mut sep = false;
        builder.push("(");
        for fd in fnames {
            if let Some(&co) = $my.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            builder.push(",");
                        }
                        builder.push(co);
                        sep = true;
                    }
                } else {
                    if sep {
                        builder.push(",");
                    }
                    builder.push(fd);
                    sep = true;
                }
            }
        }
        builder.push(") values ");

        let mut args = String::new();

        //values section
        sep = false;
        for row in $data {
            if sep {
                builder.push(",");
                args.push(' ');
            }
            builder.push("(");
            args.push_str("[ ");
            let mut sp = false;
            for (ix, fd) in fnames.iter().enumerate() {
                if let Some(&co) = $my.model.fields.get(fd) {
                    if co == "-" {
                        continue;
                    }
                }
                if sp {
                    builder.push(",");
                }
                args.push_str(&($my.to_arg)(&row[ix], &mut builder)?);
                args.push(' ');
                sp = true;
            }
            builder.push(")");
            args.push(']');
            sep = true;
        }

        //execute sql statements
        let res = match builder.build().execute($my.executor).await {
            Ok(res) => res.rows_affected(),
            Err(err) => {
                return Err($crate::anyhow!(
                    "sql:`{}` args:[{}]  {}",
                    builder.sql(),
                    args,
                    err
                ))
            }
        };

        res
    }};
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
macro_rules! sqlx_update {
    ($my:ident, $filter:ident) => {{
        let (_, fnames) = !$my.model.entity;

        let mut builder = QueryBuilder::new("update ");
        builder.push($my.model.table);
        builder.push(" set ");

        let mut args = " ".to_string();

        //update statement section
        let mut sep = false;
        for (ix, fd) in fnames.iter().enumerate() {
            if let Some(&co) = $my.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            builder.push(",");
                        }
                        builder.push(co);
                        builder.push("=");
                        args.push_str(&($my.to_arg)(&$my.model.entity[ix], &mut builder)?);
                        args.push(' ');
                        sep = true;
                    }
                } else {
                    if sep {
                        builder.push(",");
                    }
                    builder.push(fd);
                    builder.push("=");
                    args.push_str(&($my.to_arg)(&$my.model.entity[ix], &mut builder)?);
                    args.push(' ');
                    sep = true;
                }
            }
        }

        let mut idx = 0;

        //where statement section
        if $filter.expr.len() > 0 {
            builder.push(" where ");

            let mut quo = false;
            for ch in $filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= $filter.args.len() {
                        return Err($crate::anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                    args.push(' ');
                    idx += 1;
                } else {
                    builder.push(ch);
                }
            }
            debug_assert!(!quo, "where statement quotation mark not closed error");
        }

        if idx != $filter.args.len() {
            return Err($crate::anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //execute sql statements
        let res = match builder.build().execute($my.executor).await {
            Ok(res) => res.rows_affected(),
            Err(err) => {
                return Err($crate::anyhow!(
                    "sql:`{}` args:[{}]  {}",
                    builder.sql(),
                    args,
                    err
                ))
            }
        };

        res
    }};
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
macro_rules! sqlx_delete {
    ($my:ident, $filter:ident) => {{
        //from statement section
        let mut builder = QueryBuilder::new("delete from ");
        builder.push($my.model.table);

        let mut args = " ".to_string();
        let mut idx = 0;

        //where statement section
        if $filter.expr.len() > 0 {
            builder.push(" where ");

            let mut quo = false;
            for ch in $filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= $filter.args.len() {
                        return Err($crate::anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                    args.push(' ');
                    idx += 1;
                } else {
                    builder.push(ch);
                }
            }
            debug_assert!(!quo, "where statement quotation mark not closed error");
        }

        if idx != $filter.args.len() {
            return Err($crate::anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //execute sql statements
        let res = match builder.build().execute($my.executor).await {
            Ok(res) => res.rows_affected(),
            Err(err) => {
                return Err($crate::anyhow!(
                    "sql:`{}` args:[{}]  {}",
                    builder.sql(),
                    args,
                    err
                ))
            }
        };

        res
    }};
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
macro_rules! sqlx_count {
    ($my:ident, $filter:ident, $other:ident) => {{
        //determine if there is group by
        let mut builder = if matches!(&$other,Some(ot) if ot.group_by.len() > 0) {
            QueryBuilder::new("select count(*) from (select 1")
        } else {
            QueryBuilder::new("select count(*)")
        };

        //from statement section
        builder.push(" from ");
        builder.push($my.model.table);

        let mut args = " ".to_string();
        let mut idx = 0;
        let mut quo = false;

        //join on statement section
        if let Some(ot) = &$other {
            if ot.join_on.len() > 0 {
                builder.push(" ");

                for ch in ot.join_on.chars() {
                    if ch == '\'' {
                        quo = !quo;
                    }
                    if ch == '?' && !quo {
                        if idx >= $filter.args.len() {
                            return Err($crate::anyhow!(" ? exceeds the number of args"));
                        }
                        args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                        args.push(' ');
                        idx += 1;
                    } else {
                        builder.push(ch);
                    }
                }
            }
        }
        debug_assert!(!quo, "join on statement quotation mark not closed error");

        //where statement section
        if $filter.expr.len() > 0 {
            builder.push(" where ");

            for ch in $filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= $filter.args.len() {
                        return Err($crate::anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                    args.push(' ');
                    idx += 1;
                } else {
                    builder.push(ch);
                }
            }
        }
        debug_assert!(!quo, "where statement quotation mark not closed error");

        //group by statement section
        if let Some(ot) = &$other {
            if ot.group_by.len() > 0 {
                builder.push(" ");
                builder.push(ot.group_by);

                //having statement section
                if ot.having.len() > 0 {
                    builder.push(" ");

                    for ch in ot.having.chars() {
                        if ch == '\'' {
                            quo = !quo;
                        }
                        if ch == '?' && !quo {
                            if idx >= $filter.args.len() {
                                return Err($crate::anyhow!(" ? exceeds the number of args"));
                            }
                            args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                            args.push(' ');
                            idx += 1;
                        } else {
                            builder.push(ch);
                        }
                    }
                }

                builder.push(") sub");
            }
        }
        debug_assert!(!quo, "having statement quotation mark not closed error");

        if idx != $filter.args.len() {
            return Err($crate::anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //execute sql statements
        let row = match builder.build().fetch_one($my.executor).await {
            Ok(row) => row,
            Err(err) => return Err($crate::anyhow!("sql:`{}` args:[{}]  {}", builder.sql(), args, err)),
        };

        row.try_get(0)?
    }};
}

#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
macro_rules! sqlx_query {
    ($my:ident, $filter:ident, $other:ident, $limit:ident) => {{
        let (_, fnames) = !$my.model.entity;

        let mut builder = QueryBuilder::new("select ");

        //select statement section
        let mut sep = false;
        for fd in fnames {
            if let Some(&co) = $my.model.fields.get(fd) {
                if co.len() > 0 {
                    if co != "-" {
                        if sep {
                            builder.push(",");
                        }
                        builder.push(co);
                        builder.push(" as ");
                        builder.push(fd);
                        sep = true;
                    }
                } else {
                    if sep {
                        builder.push(",");
                    }
                    builder.push(fd);
                    sep = true;
                }
            }
        }

        //from statement section
        builder.push(" from ");
        builder.push($my.model.table);

        let mut args = " ".to_string();
        let mut idx = 0;
        let mut quo = false;

        //join on statement section
        if let Some(ot) = &$other {
            if ot.join_on.len() > 0 {
                builder.push(" ");

                for ch in ot.join_on.chars() {
                    if ch == '\'' {
                        quo = !quo;
                    }
                    if ch == '?' && !quo {
                        if idx >= $filter.args.len() {
                            return Err($crate::anyhow!(" ? exceeds the number of args"));
                        }
                        args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                        args.push(' ');
                        idx += 1;
                    } else {
                        builder.push(ch);
                    }
                }
            }
        }
        debug_assert!(!quo, "join on statement quotation mark not closed error");

        //where statement section
        if $filter.expr.len() > 0 {
            builder.push(" where ");

            for ch in $filter.expr.chars() {
                if ch == '\'' {
                    quo = !quo;
                }
                if ch == '?' && !quo {
                    if idx >= $filter.args.len() {
                        return Err($crate::anyhow!(" ? exceeds the number of args"));
                    }
                    args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                    args.push(' ');
                    idx += 1;
                } else {
                    builder.push(ch);
                }
            }
        }
        debug_assert!(!quo, "where statement quotation mark not closed error");

        //group by statement section
        if let Some(ot) = &$other {
            if ot.group_by.len() > 0 {
                builder.push(" ");
                builder.push(ot.group_by);

                //having statement section
                if ot.having.len() > 0 {
                    builder.push(" ");

                    for ch in ot.having.chars() {
                        if ch == '\'' {
                            quo = !quo;
                        }
                        if ch == '?' && !quo {
                            if idx >= $filter.args.len() {
                                return Err($crate::anyhow!(" ? exceeds the number of args"));
                            }
                            args.push_str(&($my.to_arg)(&*$filter.args[idx], &mut builder)?);
                            args.push(' ');
                            idx += 1;
                        } else {
                            builder.push(ch);
                        }
                    }
                }
            }
        }
        debug_assert!(!quo, "having statement quotation mark not closed error");

        if idx != $filter.args.len() {
            return Err($crate::anyhow!(
                "the quantity of ? is not equal to the number of parameters"
            ));
        }

        //order by statement section
        if $my.order.len() > 0 {
            builder.push(" order by ");
            builder.push($my.order);
        }

        //limit statement section
        if $limit {
            builder.push(" limit ");
            builder.push_bind($my.limit);
            args.push_str(&($my.limit.to_string() + " "));
            builder.push(" offset ");
            builder.push_bind($my.offset);
            args.push_str(&($my.offset.to_string() + " "));
        }

        //execute sql statements
        let rows = match builder.build().fetch_all($my.executor).await {
            Ok(rows) => rows,
            Err(err) => {
                return Err($crate::anyhow!(
                    "sql:`{}` args:[{}]  {}",
                    builder.sql(),
                    args,
                    err
                ))
            }
        };

        //convert data rows to entities
        let mut res = Vec::new();
        for row in rows {
            let mut entity = $my.model.entity.clone();
            for (ix, fd) in fnames.iter().enumerate() {
                if let Some(&co) = $my.model.fields.get(fd) {
                    if co == "-" {
                        continue;
                    }
                }
                ($my.from_row)(fd, &row, &mut entity[ix])?;
            }
            res.push(entity);
        }

        res
    }};
}
