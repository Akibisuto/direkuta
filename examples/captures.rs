extern crate direkuta;

use direkuta::prelude::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/<name:(.+)>", |_, _, c| {
                Response::new().with_body(c.get("name")).build()
            });
        }).run("0.0.0.0:3000");
}
