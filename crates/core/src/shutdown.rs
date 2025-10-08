use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use once_cell::sync::Lazy;
use tokio::sync::{broadcast, Notify};
use tracing::{info, warn};

static SHUTDOWN_COORDINATOR: Lazy<Arc<ShutdownCoordinator>> =
    Lazy::new(|| Arc::new(ShutdownCoordinator::new()));

pub struct ShutdownCoordinator {
    shutdown_sender: broadcast::Sender<()>,
    operations_complete: Arc<Notify>,
    active_operations: Arc<AtomicUsize>,
    shutdown_requested: Arc<AtomicBool>,
}

impl ShutdownCoordinator {
    fn new() -> Self {
        let (shutdown_sender, _) = broadcast::channel(16);
        Self {
            shutdown_sender,
            operations_complete: Arc::new(Notify::new()),
            active_operations: Arc::new(AtomicUsize::new(0)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_sender.subscribe()
    }

    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed)
    }

    pub fn enter_operation(self: &Arc<Self>) -> Option<OperationGuard> {
        if self.is_shutdown_requested() {
            return None;
        }
        self.active_operations.fetch_add(1, Ordering::SeqCst);
        Some(OperationGuard { coordinator: Arc::clone(self) })
    }

    fn exit_operation(&self) {
        let prev = self.active_operations.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            self.operations_complete.notify_waiters();
        }
    }

    pub async fn request_shutdown(&self, timeout: Duration) -> bool {
        info!(
            "Graceful shutdown requested, waiting for {} active operations to complete",
            self.active_operations.load(Ordering::Relaxed)
        );

        self.shutdown_requested.store(true, Ordering::SeqCst);

        let _ = self.shutdown_sender.send(());

        tokio::select! {
            _ = self.wait_for_operations_complete() => {
                info!("All operations completed gracefully");
                true
            }
            _ = tokio::time::sleep(timeout) => {
                warn!("Shutdown timeout reached, {} operations still active",
                      self.active_operations.load(Ordering::Relaxed));
                false
            }
        }
    }

    async fn wait_for_operations_complete(&self) {
        while self.active_operations.load(Ordering::Relaxed) > 0 {
            self.operations_complete.notified().await;
        }
    }

    #[cfg(test)]
    pub fn active_operations_count(&self) -> usize {
        self.active_operations.load(Ordering::Relaxed)
    }
}

pub struct OperationGuard {
    coordinator: Arc<ShutdownCoordinator>,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        self.coordinator.exit_operation();
    }
}

pub fn shutdown_coordinator() -> Arc<ShutdownCoordinator> {
    Arc::clone(&SHUTDOWN_COORDINATOR)
}

pub async fn request_graceful_shutdown(timeout: Duration) -> bool {
    shutdown_coordinator().request_shutdown(timeout).await
}

pub fn is_shutdown_in_progress() -> bool {
    shutdown_coordinator().is_shutdown_requested()
}

pub fn enter_critical_operation() -> Option<OperationGuard> {
    let coordinator = shutdown_coordinator();
    coordinator.enter_operation()
}

pub fn subscribe_to_shutdown() -> broadcast::Receiver<()> {
    shutdown_coordinator().subscribe()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Duration};
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_shutdown_coordination() {
        let coordinator = Arc::new(ShutdownCoordinator::new_for_test());

        let guard1 = coordinator.enter_operation();
        assert!(guard1.is_some());

        let guard2 = coordinator.enter_operation();
        assert!(guard2.is_some());

        assert_eq!(coordinator.active_operations_count(), 2);

        let coordinator_clone = coordinator.clone();
        let shutdown_task = tokio::spawn(async move {
            coordinator_clone.request_shutdown(Duration::from_millis(100)).await
        });

        sleep(Duration::from_millis(10)).await;

        let guard3 = coordinator.enter_operation();
        assert!(guard3.is_none());

        assert_eq!(coordinator.active_operations_count(), 2);

        drop(guard1);
        assert_eq!(coordinator.active_operations_count(), 1);

        drop(guard2);

        let result = shutdown_task.await.unwrap();
        assert!(result);
        assert_eq!(coordinator.active_operations_count(), 0);
    }

    #[tokio::test]
    async fn test_shutdown_timeout() {
        let coordinator = Arc::new(ShutdownCoordinator::new_for_test());

        let _guard = coordinator.enter_operation();
        assert!(_guard.is_some());
        assert_eq!(coordinator.active_operations_count(), 1);

        let result = coordinator.request_shutdown(Duration::from_millis(10)).await;

        assert!(!result);
        assert_eq!(coordinator.active_operations_count(), 1);

        drop(_guard);
        assert_eq!(coordinator.active_operations_count(), 0);
    }

    #[tokio::test]
    async fn test_enter_critical_operation() {
        let guard = enter_critical_operation();

        if guard.is_some() {
            drop(guard);
        }
    }

    #[tokio::test]
    async fn test_operation_guard_lifecycle() {
        let coordinator = Arc::new(ShutdownCoordinator::new_for_test());
        assert_eq!(coordinator.active_operations_count(), 0);

        {
            let _guard1 = coordinator.enter_operation();
            assert_eq!(coordinator.active_operations_count(), 1);

            {
                let _guard2 = coordinator.enter_operation();
                assert_eq!(coordinator.active_operations_count(), 2);
            }

            assert_eq!(coordinator.active_operations_count(), 1);
        }

        assert_eq!(coordinator.active_operations_count(), 0);
    }

    #[tokio::test]
    async fn test_shutdown_signal_propagation() {
        let coordinator = Arc::new(ShutdownCoordinator::new_for_test());
        let mut rx = coordinator.subscribe();

        let coordinator_clone = coordinator.clone();
        tokio::spawn(
            async move { coordinator_clone.request_shutdown(Duration::from_millis(50)).await },
        );

        let result = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }
}
