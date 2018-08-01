extern crate direkuta;

use direkuta::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/(.+)", |_, _, c| {
                let mut res = Response::new();
                res.set_body(format!("{}", c[1].1));
                res
            });
        })
        .run("0.0.0.0:8080");
}