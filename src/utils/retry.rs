//! 通用重试工具。

use std::{future::Future, time::Duration};

use tokio::time::sleep;

pub async fn retry_async<F, Fut, T, E>(
    retries: usize,
    delay: Duration,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let attempts = retries.max(1);

    for attempt in 0..attempts {
        match operation(attempt).await {
            Ok(value) => return Ok(value),
            Err(error) if attempt + 1 == attempts => return Err(error),
            Err(_) => sleep(delay).await,
        }
    }

    unreachable!("至少会执行一次重试逻辑")
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::retry_async;

    #[tokio::test]
    async fn retry_stops_after_success() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();

        let result = retry_async(3, std::time::Duration::from_millis(1), |_| {
            let counter = counter.clone();
            async move {
                let current = counter.fetch_add(1, Ordering::SeqCst);
                if current < 1 { Err("failed") } else { Ok("ok") }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
