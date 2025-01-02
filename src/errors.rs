use rocket::http::Status;

pub trait ToStatusErr<T> {
    fn status_err(self, code: u16) -> Result<T, Status>;
}

impl<T, E> ToStatusErr<T> for Result<T, E> {
    #[inline(always)]
    fn status_err(self, code: u16) -> Result<T, Status> {
        match self {
            Ok(t) => Ok(t),
            Err(_) => Err(Status::new(code)),
        }
    }
}

impl<T> ToStatusErr<T> for Option<T> {
    #[inline(always)]
    fn status_err(self, code: u16) -> Result<T, Status> {
        match self {
            Some(t) => Ok(t),
            None => Err(Status::new(code)),
        }
    }
}

impl ToStatusErr<bool> for bool {
    #[inline(always)]
    fn status_err(self, code: u16) -> Result<bool, Status> {
        match self {
            true => Ok(true),
            false => Err(Status::new(code)),
        }
    }
}
