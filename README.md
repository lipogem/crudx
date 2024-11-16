# crudx

sql crud

#### Cargo Feature Flags

- `postgres`: Using SQLX to call Postgres database

- `mysql`: Using SQLX to call MySQL database

- `sqlite`: Using SQLX to call SQLite database

- `mssql`: Using Tiberius to call MsSQL database

#### Generate data table

```sql
CREATE DATABASE school;
CREATE TABLE `oplog` (
  `pid` bigint(20) NOT NULL AUTO_INCREMENT,
  `user_id` varchar(50) CHARACTER SET utf8 NOT NULL,
  `user_ip` varchar(50) CHARACTER SET utf8 NOT NULL,
  `optime` varchar(50) CHARACTER SET utf8 NOT NULL,
  `operation` varchar(4000) CHARACTER SET utf8 NOT NULL,
  PRIMARY KEY (`pid`)
) ENGINE=InnoDB AUTO_INCREMENT=17542563 DEFAULT CHARSET=utf8;
```

#### Build entity

```rust
use struct_index::StructIndex;

#[derive(Clone, StructIndex, Debug, Default)]
pub struct Oplog {
    pub pid: i64,
    pub user_id: String,
    pub user_ip: String,
    pub optime: String,
    pub operation: String,
}
```

#### Connect to database

Refer to SQLX configuration

```rust
use sqlx::MySqlPool;

let pool = MySqlPool::connect("mysql://root:123456@127.0.0.1:3306/school")
        .await
        .unwrap();
```

#### Insert a data entry

```rust
use crudx::{
    model::{Model, Mysql},
    Executor,
};

let oplog = Oplog {
    pid: 1,
    user_id: "admin".to_string(),
    user_ip: "127.0.0.1".to_string(),
    optime: "2020-12-31 13:58".to_string(),
    operation: "Write operation document".to_string(),
};
let res = Model::new(&oplog)
    .bind(&pool)
    .insert_one(None)
    .await
    .unwrap();
println!("{}", res);
```

#### Insert some data

```rust
use crudx::{
    model::{Model, Mysql},
    Executor,
};

let res = Model::new(&Oplog::default())
    .bind(&pool)
    .insert(&[
        Oplog {
            pid: 2,
            user_id: "op1".to_string(),
            user_ip: "127.0.0.2".to_string(),
            optime: "2020-12-31 15:58".to_string(),
            operation: "Write operation document".to_string(),
        },
        Oplog {
            pid: 3,
            user_id: "op2".to_string(),
            user_ip: "127.0.0.3".to_string(),
            optime: "2020-12-31 15:59".to_string(),
            operation: "Write operation document".to_string(),
        },
    ])
    .await
    .unwrap();
println!("{}", res);
```

#### Update one piece of data

```rust
use crudx::{
    expr,
    model::{Model, Mysql},
    Executor, Filter,
};

let oplog = Oplog {
    pid: 1,
    user_id: "op0".to_string(),
    user_ip: "127.0.1.1".to_string(),
    optime: "2020-12-30 13:58".to_string(),
    operation: "Write operation document".to_string(),
};

let mut filter = Filter::default();
filter.and(expr!(oplog.pid = 1));

let res = Model::new(&oplog)
    .bind(&pool)
    .update(&filter)
    .await
    .unwrap();
println!("{}", res);
```

#### Delete data

```rust
use crudx::{
    expr,
    model::{Model, Mysql},
    Executor, Filter,
};

let oplog = Oplog::default();

let mut filter = Filter::default();
filter.and(expr!(oplog.pid = 1));

let res = Model::new(&oplog)
    .bind(&pool)
    .delete(&filter)
    .await
    .unwrap();
println!("{}", res);
```

#### Query data

```rust
use crudx::{
    expr,
    model::{Model, Mysql},
    Executor, Filter, LimitExecutor, OrderExecutor,
};

let oplog = Oplog::default();

let mut filter = Filter::default();
filter.and(expr!(oplog.optime > "2020-10-01"));
filter.and(expr!(oplog.optime < "2021-01-01"));

let res = Model::new(&oplog)
    .bind(&pool)
    .order_by("optime desc")
    .limit(&10, &0)
    .query(&filter, None)
    .await
    .unwrap();
println!("{:?}", res);
```

#### License

crudx is provided under the MIT license. See [LICENSE](LICENSE).
