#[macro_use]
extern crate direkuta;

use direkuta::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.route(Method::GET, "/", |_, _, _| {
                let mut res = Response::new().with_body("Hello World!");
                res.set_headers(headermap! {
                    header::CONTENT_TYPE => "text/plain",
                });
                res
            });
        })
        .run("0.0.0.0:3000");
}
