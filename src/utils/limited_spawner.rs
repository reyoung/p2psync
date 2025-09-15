use std::future::Future;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct LimitedSpawner {
    semaphore: Arc<Semaphore>,
}

impl LimitedSpawner {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn spawn<F>(
        &self,
        f: F,
    ) -> Result<tokio::task::JoinHandle<F::Output>, tokio::sync::AcquireError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let semaphore = Arc::clone(&self.semaphore);
        let permit = semaphore.acquire_owned().await?;
        let handle = tokio::spawn(async move {
            let _permit = permit;
            f.await
        });
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_basic_spawn() {
        let spawner = LimitedSpawner::new(2);

        let handle = spawner.spawn(async { 42 }).await.unwrap();
        let result = handle.await.unwrap();

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_concurrent_limit() {
        let spawner = LimitedSpawner::new(2);
        let counter = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        // Spawn 5 tasks, but only 2 should run concurrently
        for _ in 0..5 {
            let counter = Arc::clone(&counter);
            let max_concurrent = Arc::clone(&max_concurrent);

            let handle = spawner
                .spawn(async move {
                    let current = counter.fetch_add(1, Ordering::SeqCst) + 1;

                    // Update max concurrent if current is higher
                    let mut max_val = max_concurrent.load(Ordering::SeqCst);
                    while current > max_val {
                        match max_concurrent.compare_exchange_weak(
                            max_val,
                            current,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        ) {
                            Ok(_) => break,
                            Err(val) => max_val = val,
                        }
                    }

                    // Sleep to simulate work
                    sleep(Duration::from_millis(100)).await;

                    counter.fetch_sub(1, Ordering::SeqCst);
                    current
                })
                .await
                .unwrap();

            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Should never have more than 2 concurrent tasks
        assert!(max_concurrent.load(Ordering::SeqCst) <= 2);
    }

    #[tokio::test]
    async fn test_zero_limit() {
        let spawner = LimitedSpawner::new(0);

        // This should timeout since no permits are available
        let result =
            tokio::time::timeout(Duration::from_millis(100), spawner.spawn(async { 42 })).await;

        // Should timeout because acquire_owned() blocks indefinitely with 0 permits
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_single_permit() {
        let spawner = LimitedSpawner::new(1);
        let execution_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let mut handles = Vec::new();

        // Spawn 3 tasks with single permit - they should execute sequentially
        for i in 0..3 {
            let order = Arc::clone(&execution_order);
            let handle = spawner
                .spawn(async move {
                    order.lock().unwrap().push(i);
                    sleep(Duration::from_millis(50)).await;
                    i
                })
                .await
                .unwrap();
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap();
        }

        let final_order = execution_order.lock().unwrap();
        assert_eq!(*final_order, vec![0, 1, 2]);
    }

    #[tokio::test]
    async fn test_task_failure_releases_permit() {
        let spawner = LimitedSpawner::new(1);

        // First task panics
        let handle1 = spawner
            .spawn(async {
                panic!("Test panic");
            })
            .await
            .unwrap();

        // Wait for it to fail
        let result = handle1.await;
        assert!(result.is_err());

        // Second task should be able to run (permit should be released)
        let handle2 = spawner.spawn(async { 42 }).await.unwrap();
        let result = handle2.await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_multiple_spawners() {
        let spawner1 = LimitedSpawner::new(2);
        let spawner2 = LimitedSpawner::new(1);

        let handle1 = spawner1.spawn(async { "spawner1" }).await.unwrap();
        let handle2 = spawner2.spawn(async { "spawner2" }).await.unwrap();

        let result1 = handle1.await.unwrap();
        let result2 = handle2.await.unwrap();

        assert_eq!(result1, "spawner1");
        assert_eq!(result2, "spawner2");
    }

    #[tokio::test]
    async fn test_large_concurrent_limit() {
        let spawner = LimitedSpawner::new(100);
        let mut handles = Vec::new();

        // Spawn many tasks
        for i in 0..50 {
            let handle = spawner.spawn(async move { i * 2 }).await.unwrap();
            handles.push(handle);
        }

        // Collect results
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        // Verify all results
        results.sort();
        let expected: Vec<_> = (0..50).map(|i| i * 2).collect();
        assert_eq!(results, expected);
    }

    #[tokio::test]
    async fn test_spawn_with_different_return_types() {
        let spawner = LimitedSpawner::new(2);

        let handle_int = spawner.spawn(async { 42i32 }).await.unwrap();
        let handle_string = spawner.spawn(async { "hello".to_string() }).await.unwrap();
        let handle_bool = spawner.spawn(async { true }).await.unwrap();

        assert_eq!(handle_int.await.unwrap(), 42);
        assert_eq!(handle_string.await.unwrap(), "hello");
        assert_eq!(handle_bool.await.unwrap(), true);
    }

    #[tokio::test]
    async fn test_semaphore_fairness() {
        let spawner = LimitedSpawner::new(1);
        let execution_times = Arc::new(std::sync::Mutex::new(Vec::new()));

        let mut handles = Vec::new();

        // Spawn tasks quickly
        for i in 0..5 {
            let times = Arc::clone(&execution_times);
            let handle = spawner
                .spawn(async move {
                    let start = std::time::Instant::now();
                    sleep(Duration::from_millis(10)).await;
                    times.lock().unwrap().push((i, start));
                    i
                })
                .await
                .unwrap();
            handles.push(handle);
        }

        // Wait for completion
        for handle in handles {
            handle.await.unwrap();
        }

        // Check that tasks executed in order (FIFO)
        let mut times = execution_times.lock().unwrap();
        times.sort_by_key(|(_, time)| *time);
        let order: Vec<_> = times.iter().map(|(i, _)| *i).collect();
        assert_eq!(order, vec![0, 1, 2, 3, 4]);
    }
}
