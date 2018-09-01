#[macro_use]
extern crate direkuta;

use direkuta::prelude::hyper::*;
use direkuta::prelude::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, _, _| {
                Response::new()
                    .with_headers(headermap! {
                        header::CONTENT_TYPE => "text/plain",
                    }).with_body("Hello World!")
                    .build()
            });
        }).run("0.0.0.0:3000");
}
