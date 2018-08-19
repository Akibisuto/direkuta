extern crate direkuta;

use direkuta::prelude::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/<name:(.+)>", |_, _, c| {
                Response::new().with_body(format!("{}", c.get("name").unwrap()))
            });
        }).run("0.0.0.0:3000");
}
