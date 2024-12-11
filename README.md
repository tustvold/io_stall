# IO Stall

A toy project showing how mixed IO and CPU bound tasks impact different task executors.

## Setup

To start run nginx on port 8080 hosting the `srv` directory.

For example,

```
$ docker run -d -p 8080:80 --rm -v $PWD/srv:/usr/share/nginx/html:ro nginx
```

## Tokio IO Stall

A simple test to show how the presence of CPU bound tasks can interfere with IO performance long before
saturating the available resources

```
$ cargo run --bin tokio -- --cpu-duration 1s
   Compiling io_stall v0.1.0 (/home/raphael/repos/scratch/io_stall)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s
     Running `target/debug/tokio --cpu-duration 1s`
Average duration of 1002 ms (IO 2 ms) over 1 samples, throughput 0.99726367 rps
Average duration of 2003 ms (IO 1003 ms) over 1 samples, throughput 0.9994997 rps
Average duration of 2001 ms (IO 1000 ms) over 1 samples, throughput 0.99932754 rps
Average duration of 4003 ms (IO 3003 ms) over 1 samples, throughput 0.999542 rps
Average duration of 3001 ms (IO 2001 ms) over 1 samples, throughput 0.9998505 rps
Average duration of 6004 ms (IO 5004 ms) over 1 samples, throughput 0.9996489 rps
Average duration of 7004 ms (IO 6004 ms) over 1 samples, throughput 0.999694 rps
Average duration of 8004 ms (IO 7004 ms) over 1 samples, throughput 0.99983346 rps
Average duration of 9005 ms (IO 8004 ms) over 1 samples, throughput 0.99941564 rps
```

We can see that dispatching the blocking work off to a separate thread alleviates this

```
$ cargo run --bin tokio -- --cpu-duration 1s --io-runtime
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/tokio --cpu-duration 1s --io-runtime`
Average duration of 1002 ms (IO 2 ms) over 1 samples, throughput 0.997189 rps
Average duration of 1003 ms (IO 2 ms) over 8 samples, throughput 7.989573 rps
Average duration of 1000 ms (IO 0 ms) over 8 samples, throughput 7.9910607 rps
Average duration of 1006 ms (IO 5 ms) over 8 samples, throughput 7.9908924 rps
Average duration of 1011 ms (IO 11 ms) over 8 samples, throughput 7.9897904 rps
Average duration of 1000 ms (IO 0 ms) over 8 samples, throughput 7.991642 rps
Average duration of 1000 ms (IO 0 ms) over 8 samples, throughput 7.990861 rps
Average duration of 1000 ms (IO 0 ms) over 8 samples, throughput 7.9913797 rps
Average duration of 1000 ms (IO 0 ms) over 8 samples, throughput 7.9909844 rps
```
