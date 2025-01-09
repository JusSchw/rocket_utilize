use std::sync::Arc;

use arc_swap::ArcSwapOption;
use futures::{FutureExt, future::BoxFuture};
use rocket::{
    Build, Data, Request, Rocket, error,
    fairing::{self, Fairing, Info, Kind},
    http::ContentType,
    request::FromRequest,
    response::{self, Responder},
    serde::Serialize,
};

use crate::errors::ToStatusErr;

trait Handler: Send + Sync {
    fn handle<'r>(
        &'r self,
        req: &'r Request<'_>,
        context: &'r mut tera::Context,
        name: String,
    ) -> BoxFuture<'r, ()>;
}

impl<F, T> Handler for F
where
    F: Fn() -> T + Send + Sync,
    T: for<'r> FromRequest<'r> + Serialize + Send + Sync,
{
    fn handle<'r>(
        &'r self,
        req: &'r Request<'_>,
        context: &'r mut tera::Context,
        name: String,
    ) -> BoxFuture<'r, ()> {
        async move {
            let guard = (req.guard::<T>().await)
                .success_or_else(self)
                .unwrap_or_else(|e| e);

            context.insert(name, &guard);
        }
        .boxed()
    }
}

pub struct TemplateConfig {
    registry: Vec<(Box<dyn Handler>, String)>,
    dir: String,
}
impl TemplateConfig {
    pub fn new(dir: impl AsRef<str>) -> Self {
        Self {
            registry: Vec::new(),
            dir: dir.as_ref().into(),
        }
    }

    pub fn register<F, T>(mut self, name: impl AsRef<str>, fallback: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
        T: for<'r> FromRequest<'r> + Serialize + Send + Sync,
    {
        self.registry
            .push((Box::new(fallback), name.as_ref().into()));
        self
    }
}

#[rocket::async_trait]
impl Fairing for TemplateConfig {
    fn info(&self) -> Info {
        Info {
            name: "Tera Templates",
            kind: Kind::Ignite | Kind::Request,
        }
    }

    async fn on_ignite(&self, rocket: Rocket<Build>) -> fairing::Result {
        if let Ok(tera) = tera::Tera::new(&self.dir).inspect_err(|e| println!("{e}")) {
            Ok(rocket.manage(tera))
        } else {
            Err(rocket)
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        let mut context = tera::Context::new();
        for (handler, name) in &self.registry {
            handler.handle(req, &mut context, name.into()).await;
        }

        req.local_cache(|| ArcSwapOption::new(Some(Arc::new(context))));
    }
}

pub struct Template {
    path: String,
    context: Result<tera::Context, tera::Error>,
}

impl Template {
    pub fn render(path: impl AsRef<str>, context: impl Serialize) -> Self {
        Self {
            path: path.as_ref().into(),
            context: tera::Context::from_serialize(context),
        }
    }
}

impl<'r> Responder<'r, 'static> for Template {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        let context: &ArcSwapOption<tera::Context> = req.local_cache(ArcSwapOption::empty);

        let tera = req.rocket().state::<tera::Tera>().status_err(500)?;

        let mut context = context
            .swap(None)
            .and_then(|c| Arc::try_unwrap(c).ok())
            .status_err(500)?;

        self.context
            .map(|c| context.extend(c))
            .inspect_err(|e| error!("{e}"))
            .status_err(500)?;

        let rendered = tera
            .render(&self.path, &context)
            .inspect_err(|e| error!("{e}"))
            .status_err(500)?;

        (ContentType::HTML, rendered).respond_to(&req)
    }
}

#[macro_export]
macro_rules! context {
    ($($key:ident $(: $value:expr)?),*$(,)?) => {{
        use serde::ser::{Serialize, Serializer, SerializeMap};
        use ::std::fmt::{Debug, Formatter};
        use ::std::result::Result;

        #[allow(non_camel_case_types)]
        struct ContextMacroCtxObject<$($key: Serialize),*> {
            $($key: $key),*
        }

        #[allow(non_camel_case_types)]
        impl<$($key: Serialize),*> Serialize for ContextMacroCtxObject<$($key),*> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: Serializer,
            {
                let mut map = serializer.serialize_map(None)?;
                $(map.serialize_entry(stringify!($key), &self.$key)?;)*
                map.end()
            }
        }

        #[allow(non_camel_case_types)]
        impl<$($key: Debug + Serialize),*> Debug for ContextMacroCtxObject<$($key),*> {
            fn fmt(&self, f: &mut Formatter<'_>) -> ::std::fmt::Result {
                f.debug_struct("context!")
                    $(.field(stringify!($key), &self.$key))*
                    .finish()
            }
        }

        ContextMacroCtxObject {
            $($key $(: $value)?),*
        }
    }};
}
