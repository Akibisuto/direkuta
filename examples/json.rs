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
                let mut res = Response::new();
                res.json(|j| {
                    let hello = Example {
                        hello: String::from("world"),
                    };

                    j.body(hello);
                });
                res
            });
        }).run("0.0.0.0:3000");
}