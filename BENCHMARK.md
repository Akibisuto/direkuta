# Benchmarks

Each test was ran with the server's 'hello world' example, compile to release, and ran on a Intel i3 8100 @ 3.60GHz.

Each framework it titled along with the used commit hash.

## Table

|                 | Requests/sec   | Transfer/sec   | Latency   |
|-----------------|---------------:|---------------:|----------:|
| **Direkuta**    | 25233.36       | 2.12MB         | 8.18ms    |
| ***Hyper***     | *26540.04*     | *2.23MB*       | *7.77ms*  |
| **Actix**       | 7162.27        | 0.88MB         | 55.59ms   |
| **Gotham**      | 24155.22       | 4.45MB         | 8.63ms    |
| **Shio**        | 35017.76       | 2.97MB         | 11.38ms   |

## Direkuta (243526f2f0dae817350410dfbb785e093323fd04)

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

## Hyper (1448e4067b10da6fe4584921314afc1f5f4e3c8d)

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

## Actix (a677dc8a92003405a8f2331e6126e1f685e5acaa)

This is the only modified example, I removed all logging and left only the `/` handler.

```console
$ wrk -t10 -c400 -d30s http://0.0.0.0:8080/ --latency
Running 30s test @ http://0.0.0.0:8080/
  10 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     3.71ms    2.22ms  20.70ms   68.55%
    Req/Sec     5.56k   600.61    13.04k    72.09%
  Latency Distribution
     50%    3.35ms
     75%    5.04ms
     90%    6.75ms
     99%   10.14ms
  1659825 requests in 30.10s, 204.20MB read
Requests/sec:  55151.73
Transfer/sec:      6.78MB
```

## Gotham (d31462a1f91b900d720d035b127fa8e9ece66785)

```console
$ wrk -t10 -c400 -d30s http://0.0.0.0:7878/ --latency
Running 30s test @ http://0.0.0.0:7878/
  10 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     8.63ms    4.83ms  29.73ms   63.09%
    Req/Sec     2.44k   294.94     6.37k    75.57%
  Latency Distribution
     50%    8.03ms
     75%   12.41ms
     90%   15.48ms
     99%   19.52ms
  726705 requests in 30.08s, 133.76MB read
Requests/sec:  24155.22
Transfer/sec:      4.45MB
```

## Shio (be3c4f32b3728ce043e56af64c11e70000ba76d7)

```console
$ wrk -t10 -c400 -d30s http://0.0.0.0:7878/ --latency
Running 30s test @ http://0.0.0.0:7878/
  10 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    11.38ms  671.90us  18.32ms   88.27%
    Req/Sec     3.53k   140.61     5.61k    82.17%
  Latency Distribution
     50%   11.37ms
     75%   11.63ms
     90%   11.87ms
     99%   13.72ms
  1053481 requests in 30.08s, 89.42MB read
Requests/sec:  35017.76
Transfer/sec:      2.97MB
```
