extern crate direkuta;

use direkuta::prelude::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, _, _| Response::new().with_body("Hello World!").build());
        }).run("0.0.0.0:3000");
}
