use rocket::{Request, get, launch, request::FromRequest, routes, serde::Serialize};
use rocket_utilize::{
    context,
    template::{Template, TemplateConfig},
};

#[derive(Serialize)]
struct User {
    name: String,
    role: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();

    async fn from_request(_: &'r Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        rocket::outcome::Outcome::Success(User {
            name: "Justin".into(),
            role: "Admin".into(),
        })
    }
}

#[get("/")]
fn index() -> Template {
    Template::render("index.html", context! { rating: 100 })
}

#[launch]
fn rocket() -> _ {
    let config =
        TemplateConfig::new("templates/**/*.html").register("user", Option::<User>::default);

    rocket::build().attach(config).mount("/", routes![index])
}
