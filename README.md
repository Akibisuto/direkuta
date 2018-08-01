# Direkuta (Director)

Direkuta is a REST focused web framework for Rust. It is wrapped over top of Hyper and includes state, middleware, and routing (with parameters!).

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
            r.get("/", |_, _, _| {
                let mut res = Response::new();
                res.set_body(String::from("Hello World!"));
                res
            });
        })
        .run("0.0.0.0:3000");
}
```

## Preformance
All ran on an Intel Xeon E7 8880 (I think, its what ever Cloud9 uses).

Hyper Hello Example (Release):
```
$ wrk -t20 -c400 -d10s http://0.0.0.0:3000/
Running 10s test @ http://0.0.0.0:3000/
  20 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    44.75ms   54.83ms 361.28ms   78.74%
    Req/Sec     1.28k   584.46     4.57k    77.57%
  254404 requests in 10.16s, 21.35MB read
Requests/sec:  25034.94
Transfer/sec:      2.10MB
```

Direkuta (Release):
```
$ wrk -t20 -c400 -d10s http://0.0.0.0:8080/
Running 10s test @ http://0.0.0.0:8080/
  20 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    46.91ms   55.96ms 198.33ms   78.28%
    Req/Sec     1.08k   485.42     6.21k    82.61%
  219469 requests in 10.17s, 37.26MB read
Requests/sec:  21581.76
Transfer/sec:      3.66MB
```
