use rocket::{Request, http::Status, response};
use serde::{Deserialize, Serialize};
use serde_json::{json, value::Value};
pub trait ToStatusErr<T> {
    fn status_err(self, code: u16) -> Result<T, Status>;
}

impl<T, E> ToStatusErr<T> for Result<T, E> {
    #[inline]
    fn status_err(self, code: u16) -> Result<T, Status> {
        match self {
            Ok(t) => Ok(t),
            Err(_) => Err(Status::new(code)),
        }
    }
}

impl<T> ToStatusErr<T> for Option<T> {
    #[inline]
    fn status_err(self, code: u16) -> Result<T, Status> {
        match self {
            Some(t) => Ok(t),
            None => Err(Status::new(code)),
        }
    }
}

pub trait ToJsonError<T> {
    fn json_err(
        self,
        value: impl Serialize,
        status: impl Into<Option<Status>>,
    ) -> Result<T, ResultValue>;
}

impl<T, E> ToJsonError<T> for Result<T, E> {
    #[inline]
    fn json_err(
        self,
        value: impl Serialize,
        status: impl Into<Option<Status>>,
    ) -> Result<T, ResultValue> {
        match self {
            Ok(t) => Ok(t),
            Err(_) => Err(ResultJson::Failure(value, status).unwrap_err()),
        }
    }
}

impl<T> ToJsonError<T> for Option<T> {
    #[inline]
    fn json_err(
        self,
        value: impl Serialize,
        status: impl Into<Option<Status>>,
    ) -> Result<T, ResultValue> {
        match self {
            Some(t) => Ok(t),
            None => Err(ResultJson::Failure(value, status).unwrap_err()),
        }
    }
}

pub trait ToJsonErrorMapped<T, E> {
    fn json_err_map<S: Serialize, F: FnOnce(E) -> S>(
        self,
        f: F,
        status: impl Into<Option<Status>>,
    ) -> Result<T, ResultValue>;
}

impl<T, E> ToJsonErrorMapped<T, E> for Result<T, E> {
    fn json_err_map<S: Serialize, F: FnOnce(E) -> S>(
        self,
        f: F,
        status: impl Into<Option<Status>>,
    ) -> Result<T, ResultValue> {
        match self {
            Ok(t) => Ok(t),
            Err(err) => Err(ResultJson::Failure((f)(err), status).unwrap_err()),
        }
    }
}

pub type ResultJson = Result<ResultValue, ResultValue>;

pub trait ResultJsonExt {
    #[allow(non_snake_case)]
    fn Success(value: impl Serialize, status: impl Into<Option<Status>>) -> ResultJson {
        match serde_json::to_value(value) {
            Ok(value) => Ok(ResultValue {
                success: Some(value),
                failure: None,
                status: status.into(),
            }),
            Err(err) => Err(ResultValue {
                success: None,
                failure: Some(Value::String(err.to_string())),
                status: None,
            }),
        }
    }

    #[allow(non_snake_case)]
    fn Failure(value: impl Serialize, status: impl Into<Option<Status>>) -> ResultJson {
        match serde_json::to_value(value) {
            Ok(value) => Err(ResultValue {
                success: None,
                failure: Some(value),
                status: status.into(),
            }),
            Err(err) => Err(ResultValue {
                success: None,
                failure: Some(Value::String(err.to_string())),
                status: None,
            }),
        }
    }
}

impl ResultJsonExt for ResultJson {}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResultValue {
    pub success: Option<Value>,
    pub failure: Option<Value>,
    pub status: Option<Status>,
}

impl<'r> response::Responder<'r, 'static> for ResultValue {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        let status = self.status;
        match (self.success, self.failure) {
            (Some(success), None) => (
                status.unwrap_or(Status::new(200)),
                json!({ "success": success, "failure": null }),
            )
                .respond_to(req),
            (None, Some(failure)) => (
                status.unwrap_or(Status::new(500)),
                json!({ "success": null, "failure": failure }),
            )
                .respond_to(req),
            _ => Status::Conflict.respond_to(req),
        }
    }
}
