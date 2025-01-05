use std::time::Duration;

use tokio::time::sleep;

pub fn option_if<T>(condition: bool, value: T) -> Option<T> {
    if condition {
        Some(value)
    } else {
        None
    }
}

pub async fn sleep_ms(ms: &u64) {
    sleep(Duration::from_millis(*ms)).await
}
