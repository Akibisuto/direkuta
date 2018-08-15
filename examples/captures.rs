extern crate direkuta;

use direkuta::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/(.+)", |_, _, c| {
                Response::new().with_body(format!("{}", c[1].1))
            });
        }).run("0.0.0.0:8080");
}
