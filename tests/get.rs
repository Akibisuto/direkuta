extern crate direkuta;
extern crate tokio;
extern crate yukikaze;

use std::thread;

use direkuta::prelude::*;
use yukikaze::client::{Client, HttpClient, Request};

fn server() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, _, _| Response::new().with_body("Hello World!").build());
        }).run("0.0.0.0:3000");
}

#[test]
fn get_pass() {
    thread::spawn(move || {
        server();
    });

    let mut tokio_rt = tokio::runtime::current_thread::Runtime::new().expect("To create runtime");
    let client = Client::default();

    let request = Request::get("http://localhost:3000")
        .expect("To create get request")
        .empty();

    let response = client.execute(request);
    let response = tokio_rt.block_on(response);

    if let Ok(res) = response {
        assert!(res.is_success());
        assert_eq!(res.content_len().unwrap(), 12_u64);

        let body = res.text();
        let result = tokio_rt.block_on(body);
        assert_eq!(result.unwrap(), "Hello World!");
    }
}
