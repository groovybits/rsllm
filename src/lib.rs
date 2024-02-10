/*
 * lib.rs
 * ------
 * Author: Chris Kennedy February @2024
 *
 * This file contains the main library for the stats and network capture modules
 * for RsLLM.
*/

pub mod mpegts;
pub mod network_capture;
pub mod stream_data;
pub mod system_stats;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
pub use system_stats::{get_system_stats, SystemStats};

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

// Print a hexdump of the packet
pub fn hexdump(packet_arc: &Arc<Vec<u8>>, packet_offset: usize, packet_len: usize) {
    let packet = &packet_arc[packet_offset..packet_offset + packet_len];
    // print in rows of 16 bytes
    let mut packet_dump = String::new();
    for (i, chunk) in packet.iter().take(packet_len).enumerate() {
        if i % 16 == 0 {
            packet_dump.push_str(&format!("\n{:04x}: ", i));
        }
        packet_dump.push_str(&format!("{:02x} ", chunk));
    }
    println!(
        "--- Packet Offset {} Packet Length {} ---\n{}\n---",
        packet_offset, packet_len, packet_dump
    );
}

// return a string of the packet in hex plus ascii representation after each hex line (16 bytes) with a | delimiter
pub fn hexdump_ascii(packet: &[u8], packet_offset: usize, packet_len: usize) -> String {
    // Assuming packet_offset and packet_len are correctly calculated within the slice's bounds
    let packet = &packet[packet_offset..packet_offset + packet_len];
    let mut packet_dump = String::new();
    for (i, &chunk) in packet.iter().enumerate() {
        if i % 16 == 0 {
            packet_dump.push_str(&format!("\n{:04x}: ", i));
        }
        packet_dump.push_str(&format!("{:02x} ", chunk));
        if i % 16 == 15 || i == packet.len() - 1 {
            // Adjust for last line
            packet_dump.push_str(" | ");
            let start = if i % 16 == 15 { i - 15 } else { i / 16 * 16 };
            for &ch in &packet[start..=i] {
                if ch >= 32 && ch <= 126 {
                    packet_dump.push(ch as char);
                } else {
                    packet_dump.push('.');
                }
            }
        }
    }
    packet_dump
}
