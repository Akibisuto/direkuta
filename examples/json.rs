extern crate direkuta;
#[macro_use]
extern crate serde_derive;

use direkuta::*;

#[derive(Serialize)]
struct Example {
    hello: String,
}

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, _, _| {
                Response::new().with_json(|j| {
                    j.body(Example {
                        hello: String::from("world"),
                    });
                })
            });
        }).run("0.0.0.0:3000");
}
