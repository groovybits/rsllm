use std::time::{SystemTime, UNIX_EPOCH};

pub mod network_capture;
pub mod system_stats;
pub use system_stats::{get_system_stats, SystemStats};

use serde_json::{json, Value};

/// Enum to determine the type of stats to fetch.
pub enum StatsType {
    System,
}

/// Fetches the requested stats and returns them as a JSON Value.
pub async fn get_stats_as_json(stats_type: StatsType) -> Value {
    match stats_type {
        StatsType::System => {
            let system_stats = get_system_stats();
            json!(system_stats)
        }
    }
}

// Function to get the current Unix timestamp in milliseconds
pub fn current_unix_timestamp_ms() -> Result<u64, &'static str> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|_| "System time is before the UNIX epoch")
}
