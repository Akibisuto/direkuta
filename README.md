# Direkuta (Director)

Direkuta is a REST focused web framework for Rust. It is wrapped over top of Hyper and includes state, middleware, and routing (with parameters!).

**!!Direkuta requires Rust *Stable* 1.28 or higher!!**

**!!!Please note that Direkuta is not yet production ready, but can still be used!!!**

*Direkuta's api could change, but I wouldn't expect much to (at most the router will change).*

## Examples

Below is a simple "Hello World!" example, this was used to test benchmarks.

```rust
extern crate direkuta;

use direkuta::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, _, _| Response::new().with_body("Hello World!"));
        }).run("0.0.0.0:3000");
}
```

## Performance

All ran on an Intel i3 8100 @ 3.60GHz.

Hyper Hello Example (Release):

```console
$ wrk -t20 -c400 -d10s http://0.0.0.0:3000/
Running 10s test @ http://0.0.0.0:3000/
  20 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     8.65ms    5.94ms  41.92ms   67.63%
    Req/Sec     1.24k   252.08     3.20k    69.49%
  247483 requests in 10.09s, 20.77MB read
Requests/sec:  24537.52
Transfer/sec:      2.06MB
```

Direkuta Hello Example (Release):

```console
$ wrk -t20 -c400 -d10s http://0.0.0.0:3000/
Running 10s test @ http://0.0.0.0:3000/
  20 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     9.10ms    5.86ms  39.37ms   68.69%
    Req/Sec     1.19k   243.89     5.62k    75.16%
  237880 requests in 10.09s, 19.96MB read
Requests/sec:  23581.02
Transfer/sec:      1.98MB
```

## Middleware

Direkuta supports middleware that impliment the `Middle` trait. Direkuta comes with an example Logger middleware that can be used.

Each middleware has two states, before the response was created, and after the response has been created.

## Helpers

Direkuta comes with two features (enabled by default), HTML template support with [Tera](https://github.com/Keats/tera), and JSON support with [Serde](https://github.com/serde-rs/serde) adn [Serde JSON](https://github.com/serde-rs/json).

Tera is accessable through `State`, and uses the `templates/**/*` folder for tempaltes.

```rust
extern crate direkuta;

use direkuta::prelude::*;
use direkuta::prelude::html::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, s, _| {
                Response::new().with_body(s
                    .get::<Tera>()
                    .render(Context::new(), "index.html")
                    .unwrap())
            });
        }).run("0.0.0.0:3000");
}
```

JSON responses on the other hand are encapsulated with a `wrapper`.

Example (from `/examples`):

```rust
extern crate direkuta;
#[macro_use]
extern crate serde_derive;

use direkuta::prelude::*;

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
```

JSON Response:

```json
{
  "code": 200,
  "messages": [],
  "result": {
    "hello": "world"
  },
  "status": "OK"
}
```
