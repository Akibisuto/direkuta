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

## Preformance

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

Direkuta (Release):

```console
$ wrk -t20 -c400 -d10s http://0.0.0.0:8080/
Running 10s test @ http://0.0.0.0:3000/
  20 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     9.17ms    5.97ms  41.94ms   68.59%
    Req/Sec     1.18k   218.47     2.79k    69.75%
  234837 requests in 10.10s, 19.71MB read
Requests/sec:  23254.98
Transfer/sec:      1.95MB
```
