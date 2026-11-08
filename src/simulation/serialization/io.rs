//! Disk, postcard binary, and clipboard seed string I/O for system states.

use std::path::Path;

use super::types::{SystemSaveData, SEED_PREFIX};

/// Encodes `SystemSaveData` into a version-prefixed JSON system seed string suitable for sharing and clipboard exchange.
pub fn encode_system_seed_json(data: &SystemSaveData) -> Result<String, String> {
    let json = serde_json::to_string(data).map_err(|e| e.to_string())?;
    Ok(format!("{SEED_PREFIX}{json}"))
}

/// Decodes a system seed string (supporting either prefixed or raw JSON) into `SystemSaveData`.
pub fn decode_system_seed_json(seed: &str) -> Result<SystemSaveData, String> {
    let trimmed = seed.trim();
    let json_str = if let Some(stripped) = trimmed.strip_prefix(SEED_PREFIX) {
        stripped.trim()
    } else {
        trimmed
    };
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

/// Scans the `saves/` and workspace directory for save files (`.json`, `.bin`, `.postcard`)
/// and returns sorted unique slot stems (e.g. `["quicksave", "trappist_custom"]`).
pub fn list_save_slots() -> Vec<String> {
    let mut slots = Vec::new();
    let dirs = [Path::new("saves"), Path::new(".")];
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if matches!(ext, "json" | "bin" | "postcard") {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                if !slots.contains(&stem.to_string()) {
                                    slots.push(stem.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    slots.sort();
    if slots.is_empty() {
        slots.push("quicksave".to_string());
    }
    slots
}

/// Normalizes a save filename, ensuring it points into the `saves/` folder if no path separators are present.
pub fn normalize_save_path(filename: &str) -> String {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        "saves/quicksave.json".to_string()
    } else if trimmed.contains('/') || trimmed.contains('\\') {
        trimmed.to_string()
    } else {
        format!("saves/{trimmed}")
    }
}

#[inline]
pub fn is_binary_save_path(path_str: &str) -> bool {
    Path::new(path_str)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("bin") || ext.eq_ignore_ascii_case("postcard"))
}

/// Serializes a `SystemSaveData` snapshot into compact, bit-exact `postcard` binary bytes.
pub fn serialize_system_postcard(data: &SystemSaveData) -> Result<Vec<u8>, std::io::Error> {
    postcard::to_allocvec(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

/// Deserializes a `SystemSaveData` snapshot from compact, bit-exact `postcard` binary bytes.
pub fn deserialize_system_postcard(bytes: &[u8]) -> Result<SystemSaveData, std::io::Error> {
    postcard::from_bytes(bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

/// Saves a `SystemSaveData` snapshot to the specified JSON (`.json`) or binary (`.bin`/`.postcard`) file path.
pub fn save_system_to_file(data: &SystemSaveData, path_str: &str) -> Result<(), std::io::Error> {
    let path = Path::new(path_str);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if is_binary_save_path(path_str) {
        let bytes = serialize_system_postcard(data)?;
        std::fs::write(path, bytes)?;
    } else {
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)?;
    }
    Ok(())
}

/// Loads a `SystemSaveData` snapshot from the specified JSON (`.json`) or binary (`.bin`/`.postcard`) file path.
pub fn load_system_from_file(path_str: &str) -> Result<SystemSaveData, std::io::Error> {
    if is_binary_save_path(path_str) {
        let bytes = std::fs::read(path_str)?;
        deserialize_system_postcard(&bytes)
    } else {
        let content = std::fs::read_to_string(path_str)?;
        let data: SystemSaveData = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(data)
    }
}
