use std::future::Future;

use crate::{Filter, Result};

pub struct Other<'a> {
    /// join on statement section, need to have join on
    /// # Example
    /// ```no_run
    /// "left join class on class.id=student.class_id"
    /// ```
    pub join_on: &'a str,
    /// group by statement section, need to have group by
    /// # Example
    /// ```no_run
    /// "group by name"
    /// ```
    pub group_by: &'a str,
    /// having statement section, need to have having
    /// # Example
    /// ```no_run
    /// "having max(age)<35"
    /// ```
    pub having: &'a str,
}

pub trait Executor<'a, T> {
    /// insert a piece of data into the database
    /// # Example
    /// ```no_run
    /// let clazz = Clazz {
    ///     id: 0,
    ///     name: "one".to_string(),
    /// };
    /// let mut model = Model::new(&clazz);
    /// *model.fields.get_mut(field!(clazz.id)).unwrap() = "-";
    /// let res = model
    ///     .bind(&pool)
    ///     .insert_one(Some(&Filter::default().and((
    ///         "not exists(select 1 from clazz where name=?)",
    ///         args!("one"),
    ///     ))))
    ///     .await
    ///     .unwrap();
    /// ```
    fn insert_one(self, filter: Option<&'a Filter>) -> impl Future<Output = Result<u64>> + Send;

    /// insert some data into the database
    /// # Example
    /// ```no_run
    /// let clazz = Clazz::default();
    /// let mut model = Model::new(&clazz);
    /// *model.fields.get_mut(field!(clazz.id)).unwrap() = "-";
    /// let res = model
    ///     .bind(&pool)
    ///     .insert(&vec![
    ///         Clazz {
    ///             id: 0,
    ///             name: "one".to_string(),
    ///         },
    ///         Clazz {
    ///             id: 0,
    ///             name: "two".to_string(),
    ///         },
    ///         Clazz {
    ///             id: 0,
    ///             name: "three".to_string(),
    ///         },
    ///     ])
    ///     .await
    ///     .unwrap();
    /// ```
    fn insert(self, data: &'a [T]) -> impl Future<Output = Result<u64>> + Send;

    /// update eligible data to the database
    /// # Example
    /// ```no_run
    /// let clazz = Clazz {
    ///     id: 0,
    ///     name: "five".to_string(),
    /// };
    /// let mut model = Model::new(&clazz);
    /// *model.fields.get_mut(field!(clazz.id)).unwrap() = "-";
    /// let res = model
    ///     .bind(&pool)
    ///     .update(Filter::default().and(expr!(clazz.id > 5)))
    ///     .await
    ///     .unwrap();
    /// ```
    fn update(self, filter: &'a Filter) -> impl Future<Output = Result<u64>> + Send;

    /// delete data from the database that meets the criteria
    /// # Example
    /// ```no_run
    /// let clazz = Clazz::default();
    /// let res = Model::new(&clazz)
    ///     .bind(&pool)
    ///     .delete(Filter::default().and(expr!(clazz.name = "five")))
    ///     .await
    ///     .unwrap();
    /// ```
    fn delete(self, filter: &'a Filter) -> impl Future<Output = Result<u64>> + Send;

    /// obtain the number of database data records
    /// # Example
    /// ```no_run
    /// let res = Model::new(&Clazz::default())
    ///     .bind(&pool)
    ///     .count(
    ///         &Filter::default().and(("", args!(1))),
    ///         Some(Other {
    ///             join_on: "",
    ///             group_by: "group by name",
    ///             having: "having count(*)>?",
    ///         }),
    ///     )
    ///     .await
    ///     .unwrap();
    /// ```
    fn count(
        self,
        filter: &'a Filter,
        other: Option<Other<'a>>,
    ) -> impl Future<Output = Result<i64>> + Send;

    /// order by statement section  
    /// !!please note that there is an injection risk when using upload fields
    fn order_by(self, order: &'a str) -> impl OrderExecutor<'a, T>;
}

pub trait OrderExecutor<'a, T> {
    /// query a piece of data in the database
    /// # Example
    /// ```no_run
    /// let student = Student::default();
    /// let mut model = Model::new(&student);
    /// *model.fields.get_mut(field!(student.clazz_name)).unwrap() = "-";
    /// let res = model
    ///     .bind(&pool)
    ///     .order_by(field!(student.id))
    ///     .query_one(&Filter::default(), None)
    ///     .await
    ///     .unwrap();
    /// ```
    fn query_one(
        self,
        filter: &'a Filter,
        other: Option<Other<'a>>,
    ) -> impl Future<Output = Result<T>> + Send;

    /// query some data in the database
    /// # Example
    /// ```no_run
    /// let student = Student::default();
    /// let mut model = Model::new(&student);
    /// *model.fields.get_mut(field!(student.id)).unwrap() = "student.id";
    /// *model.fields.get_mut(field!(student.name)).unwrap() = "student.name";
    /// *model.fields.get_mut(field!(student.clazz_name)).unwrap() = "clazz.name";
    /// let res = model
    ///     .bind(&pool)
    ///     .order_by("student.id")
    ///     .query(
    ///         &Filter::default(),
    ///         Some(Other {
    ///             join_on: "left join clazz on clazz.id=student.clazz_id",
    ///             group_by: "",
    ///             having: "",
    ///         }),
    ///     )
    ///     .await
    ///     .unwrap();
    /// ```
    fn query(
        self,
        filter: &'a Filter,
        other: Option<Other<'a>>,
    ) -> impl Future<Output = Result<Vec<T>>> + Send;

    /// limit statement section
    fn limit(self, limit: &'a i64, offset: &'a i64) -> impl LimitExecutor<'a, T>;
}

pub trait LimitExecutor<'a, T> {
    /// perform a paginated query on the data in the database
    /// # Example
    /// ```no_run
    /// let student = Student::default();
    /// let mut model = Model::new(&student);
    /// *model.fields.get_mut(field!(student.clazz_name)).unwrap() = "-";
    /// let res = model
    ///     .bind(&pool)
    ///     .order_by(field!(student.id))
    ///     .limit(&3, &1)
    ///     .query(&Filter::default(), None)
    ///     .await
    ///     .unwrap();
    /// ```
    fn query(
        self,
        filter: &'a Filter,
        other: Option<Other<'a>>,
    ) -> impl Future<Output = Result<Vec<T>>> + Send;
}
