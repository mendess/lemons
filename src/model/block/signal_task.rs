use crate::util::signal::sig_rt_min;
use std::{future::Future, io, os::raw::c_int};
use tokio::signal::unix::{signal, SignalKind};

pub async fn run<F, Fut, T>(n: c_int, data: T, mut cont: F) -> io::Result<()>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = bool>,
    T: Clone,
{
    let mut signals = signal(SignalKind::from_raw(sig_rt_min() + n))?;
    while cont(data.clone()).await && signals.recv().await.is_some() {}
    {}
    Ok(())
}

pub async fn do_and_run<F, Fut, T>(n: c_int, data: T, mut cont: F) -> io::Result<()>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = bool>,
    T: Clone,
{
    if !cont(data.clone()).await {
        Ok(())
    } else {
        run(n, data, cont).await
    }
}
