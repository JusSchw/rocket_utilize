use std::{marker::PhantomData, sync::Arc};

use arc_swap::ArcSwapOption;
use futures::{FutureExt, future::BoxFuture};
use rocket::{
    Build, Data, Request, Rocket, error,
    fairing::{self, Fairing, Info, Kind},
    http::{ContentType, Status},
    request::FromRequest,
    response::{self, Responder},
    serde::Serialize,
};

trait Handler: Send + Sync {
    fn handle<'r>(
        &self,
        req: &'r Request<'_>,
        context: &'r mut tera::Context,
        name: String,
    ) -> BoxFuture<'r, ()>;
}

struct Phantom<T> {
    _marker: PhantomData<T>,
}

impl<T> Phantom<T> {
    fn new() -> Self {
        Phantom {
            _marker: PhantomData,
        }
    }
}

impl<T> Handler for Phantom<T>
where
    T: for<'r> FromRequest<'r> + Default + Serialize + Send + Sync,
{
    fn handle<'r>(
        &self,
        req: &'r Request<'_>,
        context: &'r mut tera::Context,
        name: String,
    ) -> BoxFuture<'r, ()> {
        async move {
            let guard = (req.guard::<T>().await)
                .success_or_else(T::default)
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

    pub fn register<T>(mut self, name: impl AsRef<str>) -> Self
    where
        T: for<'r> FromRequest<'r> + Default + Serialize + Send + Sync + 'static,
    {
        self.registry
            .push((Box::new(Phantom::<T>::new()), name.as_ref().into()));
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

        let tera = req
            .rocket()
            .state::<tera::Tera>()
            .ok_or(Status::InternalServerError)?;

        let mut context = context
            .swap(None)
            .and_then(|c| Arc::try_unwrap(c).ok())
            .ok_or(Status::InternalServerError)?;

        self.context.map(|c| context.extend(c)).map_err(|e| {
            error!("{e}");
            Status::InternalServerError
        })?;

        let rendered = tera.render(&self.path, &context).map_err(|e| {
            error!("{e}");
            Status::InternalServerError
        })?;

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
