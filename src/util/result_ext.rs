pub trait ResultExt<T> {
    fn merge(self) -> T;
}

impl<T> ResultExt<T> for Result<T, T> {
    fn merge(self) -> T {
        match self {
            Ok(t) => t,
            Err(e) => e,
        }
    }
}
