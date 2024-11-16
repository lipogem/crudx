use std::any::Any;

#[derive(Debug)]
pub struct Filter {
    pub expr: String,
    pub args: Vec<Box<dyn Any + Send + Sync>>,
}

impl Filter {
    /// and conditional expressions
    /// # Example
    /// ```no_run
    /// let name = "Alice".to_string();
    /// filter.if_and(name.len() > 0, ("name like ?", args!(name + "%")));
    /// ```
    pub fn if_and(
        &mut self,
        condition: bool,
        item: (&str, Vec<Box<dyn Any + Send + Sync>>),
    ) -> &mut Self {
        if condition {
            if item.0.len() > 0 {
                if self.expr.len() > 0 {
                    self.expr.push_str(" and ");
                }
                self.expr.push_str(item.0);
            }
            if item.1.len() > 0 {
                self.args.extend(item.1);
            }
        }
        self
    }

    /// and expressions
    /// # Example
    /// ```no_run
    /// filter.and(expr!(student.id = 18));
    /// ```
    pub fn and(&mut self, item: (&str, Vec<Box<dyn Any + Send + Sync>>)) -> &mut Self {
        self.if_and(true, item)
    }

    /// or conditional expressions
    /// # Example
    /// ```no_run
    /// let name = "Alice".to_string();
    /// filter.if_or(
    ///     name.len() > 0,
    ///     ("name like ?", args!(format!("%{}%", name))),
    /// );
    /// ```
    pub fn if_or(
        &mut self,
        condition: bool,
        item: (&str, Vec<Box<dyn Any + Send + Sync>>),
    ) -> &mut Self {
        if condition {
            if item.0.len() > 0 {
                if self.expr.len() > 0 {
                    self.expr.push_str(" or ");
                }
                self.expr.push_str(item.0);
            }
            if item.1.len() > 0 {
                self.args.extend(item.1);
            }
        }
        self
    }

    /// or expressions
    /// # Example
    /// ```no_run
    /// filter.or(expr!(student.name like "Al%"));
    /// ```
    pub fn or(&mut self, item: (&str, Vec<Box<dyn Any + Send + Sync>>)) -> &mut Self {
        self.if_or(true, item)
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            expr: String::new(),
            args: Vec::new(),
        }
    }
}

/// get field name
/// # Example
/// ```no_run
/// assert_eq!(field!(student.name), "name");
/// ```
#[macro_export]
macro_rules! field {
    ($struct_value:ident.$field:ident) => {{
        let _ = &$struct_value.$field;
        stringify!($field)
    }};
}

/// conditional expression parameters
/// # Example
/// ```no_run
/// assert_eq!(
///     args!("Alice", "Mary"),
///     vec![Box::new("Alice"), Box::new("Mary")]
/// );
/// ```
#[macro_export]
macro_rules! args {
    ($($arg:expr),* $(,)?) => {{
        vec![$(Box::new($arg)),*]
    }};
}

/// generate a conditional expression
/// # Example
/// ```no_run
/// assert_eq!(
///     expr!(student.name = "Alice"),
///     (&"name = ?".to_string(), vec![Box::new("Alice")])
/// );
/// ```
#[macro_export]
macro_rules! expr {
    ($struct_value:ident.$field:ident $op:tt $arg:expr) => {{
        let _ = &$struct_value.$field;
        (
            &format!("{} {} ?", stringify!($field), stringify!($op)),
            vec![Box::new($arg)],
        )
    }};
}
