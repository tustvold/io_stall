use clap::Parser;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;

/// A simple test program that does some IO followed by some blocking CPU-bound work
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Whether to use a separate IO runtime
    #[clap(long)]
    io_runtime: bool,

    #[clap(long, value_parser = humantime::parse_duration, default_value = "10ms")]
    cpu_duration: Duration,

    #[clap(long, default_value = "4")]
    concurrency: usize,
}

async fn do_io(client: &Client) {
    let resp = client
        .get("http://localhost:8080/random.img")
        .send()
        .await
        .unwrap();

    let out = resp
        .bytes_stream()
        .try_fold(0, |acc, b| futures::future::ready(Ok(acc + b.len())))
        .await
        .unwrap();

    assert_eq!(out, 1048576);
}

async fn do_cpu(duration: Duration, blocking_runtime: Option<&Handle>) {
    use std::thread::sleep;
    match blocking_runtime {
        Some(x) => x.spawn(async move { sleep(duration) }).await.unwrap(),
        None => sleep(duration),
    }
}

async fn do_work(
    client: Client,
    cpu_duration: Duration,
    df_runtime: Option<&Handle>,
) -> (Duration, Duration) {
    let start = Instant::now();
    do_io(&client).await;
    let io = start.elapsed();
    do_cpu(cpu_duration, df_runtime).await;
    (io, start.elapsed())
}

#[tokio::main(worker_threads = 8)]
async fn main() {
    let client = Client::new();
    let args = Args::parse();

    let blocking_runtime = args.io_runtime.then(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(8)
            .build()
            .unwrap()
    });

    let blocking_runtime = blocking_runtime.as_ref().map(|x| x.handle());
    let mut s = futures::stream::iter(std::iter::from_fn(|| {
        Some(do_work(client.clone(), args.cpu_duration, blocking_runtime))
    }))
    .buffer_unordered(8);

    let mut last_output = Instant::now();
    let mut buf = vec![];
    while let Some(n) = s.next().await {
        buf.push(n);
        let elapsed = last_output.elapsed().as_secs_f32();
        if elapsed >= 1. {
            let io: u128 = buf.iter().map(|(x, _)| x.as_millis()).sum();
            let query: u128 = buf.iter().map(|(_, x)| x.as_millis()).sum();
            println!(
                "Average duration of {} ms (IO {} ms) over {} samples, throughput {} rps",
                query / buf.len() as u128,
                io / buf.len() as u128,
                buf.len(),
                buf.len() as f32 / elapsed
            );
            buf.clear();
            last_output = Instant::now();
        }
    }

    tokio::signal::ctrl_c().await.unwrap();
}
