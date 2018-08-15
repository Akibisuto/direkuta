extern crate direkuta;

use direkuta::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, _, _| {
                let mut res = Response::new();
                res.set_body("Hello World!");
                res
            });
        }).run("0.0.0.0:3000");
}
