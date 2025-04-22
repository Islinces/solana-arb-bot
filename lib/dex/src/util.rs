use std::future::Future;
use tokio::task::JoinHandle;

#[inline(always)]
#[allow(unused_variables)]
pub fn tokio_spawn<T>(name: &str, future: T) -> JoinHandle<T::Output>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    #[cfg(not(tokio_unstable))]
    {
        tokio::spawn(future)
    }

    #[cfg(tokio_unstable)]
    {
        tokio::task::Builder::new()
            .name(name)
            .spawn(future)
            .expect("always Ok")
    }
}

/// Panics if the local time is < unix epoch start
pub fn millis_since_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}


