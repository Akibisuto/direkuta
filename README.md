# Direkuta (Director)

Direkuta is a REST focused web framework for Rust. It is wrapped over top of Hyper and includes state, middleware, and routing (with parameters!).

**!!Direkuta requires Rust *Stable* 1.28 or higher!!**

**!!!Please note that Direkuta is not yet production ready, but can still be used!!!**

*Direkuta's api could change, but I wouldn't expect much to (at most the router will change).*

## Examples

Below is a simple "Hello World!" example, this was used to test benchmarks.

```rust
extern crate direkuta;

use direkuta::prelude::*;

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
    Latency     8.07ms    5.44ms  42.40ms   65.97%
    Req/Sec     1.33k   246.38     5.66k    78.02%
  265182 requests in 10.10s, 22.25MB read
Requests/sec:  26267.59
Transfer/sec:      2.20MB
```

Direkuta Hello Example (Release):

```console
$ wrk -t20 -c400 -d10s http://0.0.0.0:3000/
Running 10s test @ http://0.0.0.0:3000/
  20 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     8.52ms    5.34ms  39.38ms   66.26%
    Req/Sec     1.26k   203.46     4.36k    77.51%
  253031 requests in 10.10s, 21.24MB read
Requests/sec:  25049.60
Transfer/sec:      2.10MB
```

## Middleware

Direkuta supports middleware that impliment the `Middle` trait. Direkuta comes with an example Logger middleware that can be used.

Each middleware has two states, before the response was created, and after the response has been created.

## Helpers

Direkuta comes with two features (enabled by default), HTML template support with [Tera](https://github.com/Keats/tera), and JSON support with [Serde](https://github.com/serde-rs/serde) and [Serde JSON](https://github.com/serde-rs/json).

Tera is accessable through `State`, and uses the `templates/**/*` folder for tempaltes.

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
                Response::new().with_body(c.get("name").unwrap().as_str())
            });
        }).run("0.0.0.0:3000");
}
```

The routing system also has paths which allow you to group other paths under a section of the url.

Soon their will also be a group which allows you to group handlers under one path.
