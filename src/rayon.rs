use async_task::Runnable;
use clap::Parser;
use futures::stream::TryStreamExt;
use futures::{FutureExt, StreamExt};
use reqwest::Client;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// A simple test program that does some IO followed by some blocking CPU-bound work
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
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

fn do_cpu(duration: Duration) {
    std::thread::sleep(duration)
}

async fn do_work(client: Client, cpu_duration: Duration) -> (Duration, Duration) {
    let start = Instant::now();
    do_io(&client).await;
    let io = start.elapsed();
    do_cpu(cpu_duration);
    (io, start.elapsed())
}

/// An executor designed to run potentially blocking futures
struct AsyncExecutor {
    io: tokio::runtime::Handle,
    cpu: Arc<rayon::ThreadPool>,
}

impl AsyncExecutor {
    pub fn new() -> Self {
        let io = tokio::runtime::Handle::current();
        let cpu = rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .use_current_thread()
            .build()
            .unwrap();

        let cpu = Arc::new(cpu);
        Self { io, cpu }
    }

    pub fn spawn<F>(&self, fut: F) -> SpawnHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let handle = self.io.clone();

        // This box is technically unnecessary, but avoids some pin shenanigans
        let mut boxed = Box::pin(fut);

        // Enter tokio runtime whilst polling future - allowing IO and timers to work
        let io_fut = futures::future::poll_fn(move |cx| {
            let _guard = handle.enter();
            boxed.poll_unpin(cx)
        });
        // Route result back to oneshot
        let remote_fut = io_fut.map(|out| {
            let _ = sender.send(out);
        });

        // Task execution is scheduled on rayon
        let cpu = self.cpu.clone();
        let (runnable, task) = async_task::spawn(remote_fut, move |runnable: Runnable<()>| {
            cpu.spawn(move || {
                let _ = runnable.run();
            });
        });
        runnable.schedule();
        SpawnHandle {
            _task: task,
            receiver,
        }
    }
}

/// Handle returned by [`AsyncExecutor`]
///
/// Cancels task on drop
struct SpawnHandle<T> {
    receiver: futures::channel::oneshot::Receiver<T>,
    _task: async_task::Task<()>,
}

impl<T> Future for SpawnHandle<T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.poll_unpin(cx).map(|x| x.ok())
    }
}

#[tokio::main(worker_threads = 8)]
async fn main() {
    let client = Client::new();
    let args = Args::parse();

    let runtime = AsyncExecutor::new();

    let mut s = futures::stream::iter(std::iter::from_fn(|| {
        let client = client.clone();
        Some(runtime.spawn(do_work(client, args.cpu_duration)))
    }))
    .buffer_unordered(args.concurrency);

    let mut last_output = Instant::now();
    let mut buf = vec![];
    while let Some(n) = s.next().await {
        buf.push(n);
        let elapsed = last_output.elapsed().as_secs_f32();
        if elapsed >= 1. {
            let io: u128 = buf.iter().map(|x| x.as_ref().unwrap().0.as_millis()).sum();
            let query: u128 = buf.iter().map(|x| x.as_ref().unwrap().1.as_millis()).sum();
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
}
