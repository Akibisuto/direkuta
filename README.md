# Direkuta (Director)

Direkuta is a REST focused web framework for Rust. It is wrapped over top of Hyper and includes state, middleware, and routing (with parameters!).

## Examples

Below is a simple "Hello World!" example, this was used to test benchmarks.

```rust
extern crate direkuta;

use direkuta::prelude::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, _, _| Response::new().with_body("Hello World!").build());
        }).run("0.0.0.0:3000");
}
```

## Performance

All ran on an Intel i3 8100 @ 3.60GHz.

Hyper Hello Example (Release):

```console
$ wrk -t10 -c400 -d30s http://0.0.0.0:3000/ --latency
Running 30s test @ http://0.0.0.0:3000/
  10 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     7.77ms    4.63ms  31.50ms   64.66%
    Req/Sec     2.67k   328.58     5.49k    71.73%
  Latency Distribution
     50%    7.14ms
     75%   11.16ms
     90%   14.34ms
     99%   18.95ms
  798726 requests in 30.10s, 67.03MB read
Requests/sec:  26540.04
Transfer/sec:      2.23MB
```

Direkuta Hello Example (Release):

```console
$ wrk -t10 -c400 -d30s http://0.0.0.0:3000/ --latency
Running 30s test @ http://0.0.0.0:3000/
  10 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     8.13ms    4.67ms  29.20ms   63.54%
    Req/Sec     2.57k   312.47     7.09k    74.02%
  Latency Distribution
     50%    7.58ms
     75%   11.66ms
     90%   14.72ms
     99%   19.08ms
  765359 requests in 30.09s, 64.23MB read
Requests/sec:  25436.42
Transfer/sec:      2.13MB
```

## Middleware

Direkuta supports middleware that implement the `Middle` trait. Direkuta comes with an example Logger middleware that can be used.

Each middleware has two states, before the response was created, and after the response has been created.

## Helpers

Direkuta comes with two features (enabled by default), HTML template support with [Tera](https://github.com/Keats/tera), and JSON support with [Serde](https://github.com/serde-rs/serde) and [Serde JSON](https://github.com/serde-rs/json).

Tera is accessible through `State`, and uses the `templates/**/*` folder for templates.

```rust
extern crate direkuta;

use direkuta::prelude::*;sp
use direkuta::prelude::html::*;

fn main() {
    Direkuta::new()
        .route(|r| {
            r.get("/", |_, s, _| {
                Response::new().with_body(s
                    .get::<Tera>()
                    .render(Context::new(), "index.html")
                    .unwrap()).build()
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
                }).build()
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

## Routing

Direkuta has a ID/Regex based routing system in the format of `/<name:(.*)>/`, the capture from the request can later be accessed with `c.get("name")`.

Like so (from `/examples`):

```rust
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
```

The routing system also has paths which allow you to group other paths under a section of the url.

Soon their will also be a group which allows you to group handlers under one path.
