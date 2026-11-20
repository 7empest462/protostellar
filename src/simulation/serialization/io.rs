//! Disk, postcard binary, and clipboard seed string I/O for system states.

use std::path::Path;

use super::types::{SEED_PREFIX, SystemSaveData};

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

/// Metadata summary for a saved solar system slot on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveSlotEntry {
    pub slot_name: String,
    pub file_path: String,
    pub scenario_preset: Option<crate::simulation::scenarios::ScenarioPreset>,
    pub body_count: usize,
    pub timestamp_epoch_yr: f64,
    pub step_count: u64,
    pub format_tag: &'static str,
    pub modified_secs: u64,
}

const SAVE_FILE_EXTENSIONS: [&str; 3] = ["json", "postcard", "bin"];

fn matching_save_extension(path_str: &str) -> Option<&'static str> {
    let ext = Path::new(path_str).extension()?.to_str()?;
    SAVE_FILE_EXTENSIONS
        .iter()
        .copied()
        .find(|allowed| ext.eq_ignore_ascii_case(allowed))
}

/// Sanitizes an arbitrary save slot name into a safe single-segment file stem.
/// Strips redundant `.json`/`.postcard`/`.bin` extensions and replaces unsafe characters.
pub fn sanitize_save_slot_name(raw: &str) -> String {
    let mut stem = raw.trim().to_string();
    while let Some(ext) = matching_save_extension(&stem) {
        let strip_len = ext.len().saturating_add(1);
        stem.truncate(stem.len().saturating_sub(strip_len));
    }

    let mut out = String::with_capacity(stem.len());
    let mut prev_underscore = false;
    for ch in stem.chars() {
        let safe = ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-');
        if safe {
            out.push(ch);
            prev_underscore = false;
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
        }
    }
    while out.ends_with('_') || out.ends_with('.') {
        out.pop();
    }
    if out.len() > 48 {
        let mut cut = 48;
        while cut > 0 && !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        while out.ends_with('_') {
            out.pop();
        }
    }
    if out.is_empty() {
        "quicksave".to_string()
    } else {
        out
    }
}

/// Resolves a user-entered slot name (or `.postcard`/`.bin`/`.json` filename) to a write path in `saves/`.
pub fn resolve_save_path_for_write(slot_or_file: &str) -> String {
    let trimmed = slot_or_file.trim();
    let ext = matching_save_extension(trimmed).unwrap_or("json");
    let sanitized = sanitize_save_slot_name(trimmed);
    format!("saves/{sanitized}.{ext}")
}

/// Resolves a slot name or filename for loading, preferring exact existing files then `saves/<stem>.{json,postcard,bin}`.
pub fn resolve_save_path_for_read(slot_or_file: &str) -> String {
    let trimmed = slot_or_file.trim();
    if trimmed.contains('/') || trimmed.contains('\\') {
        return normalize_save_path(trimmed);
    }
    if !trimmed.is_empty() {
        let direct_in_saves = format!("saves/{trimmed}");
        if Path::new(&direct_in_saves).is_file() {
            return direct_in_saves;
        }
        if Path::new(trimmed).is_file() {
            return trimmed.to_string();
        }
    }

    let stem = sanitize_save_slot_name(trimmed);
    for dir in ["saves", "."] {
        for ext in SAVE_FILE_EXTENSIONS {
            let candidate = if dir == "." {
                format!("{stem}.{ext}")
            } else {
                format!("{dir}/{stem}.{ext}")
            };
            if Path::new(&candidate).is_file() {
                return candidate;
            }
        }
    }
    format!("saves/{stem}.json")
}

fn format_tag_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("bin") || ext.eq_ignore_ascii_case("postcard") => {
            "BIN"
        }
        _ => "JSON",
    }
}

fn modified_unix_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |dur| dur.as_secs())
}

fn read_save_slot_entry(path: &Path) -> Option<SaveSlotEntry> {
    if !path.is_file() {
        return None;
    }
    let ext = path.extension().and_then(|s| s.to_str())?;
    if !SAVE_FILE_EXTENSIONS
        .iter()
        .any(|allowed| ext.eq_ignore_ascii_case(allowed))
    {
        return None;
    }
    let stem = path.file_stem().and_then(|s| s.to_str())?;
    let file_path = path.to_string_lossy().into_owned();
    let data = load_system_from_file(&file_path).ok()?;
    Some(SaveSlotEntry {
        slot_name: stem.to_string(),
        file_path,
        scenario_preset: data.scenario_preset,
        body_count: data.bodies.len(),
        timestamp_epoch_yr: data.timestamp_epoch_yr,
        step_count: data.step_count,
        format_tag: format_tag_for_path(path),
        modified_secs: modified_unix_secs(path),
    })
}

/// Collects valid saved system entries from a specific directory, sorted newest-first.
pub fn list_save_slot_entries_in_dir(dir: &Path) -> Vec<SaveSlotEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return entries;
    };
    for dir_entry in read_dir.flatten() {
        if let Some(slot_entry) = read_save_slot_entry(&dir_entry.path()) {
            entries.push(slot_entry);
        }
    }
    entries.sort_by(|a, b| {
        b.modified_secs
            .cmp(&a.modified_secs)
            .then_with(|| a.slot_name.cmp(&b.slot_name))
    });
    entries
}

/// Scans `saves/` and the workspace root for valid saved system files, sorted newest-first.
pub fn list_save_slot_entries() -> Vec<SaveSlotEntry> {
    let mut entries = list_save_slot_entries_in_dir(Path::new("saves"));
    for root_entry in list_save_slot_entries_in_dir(Path::new(".")) {
        let already_listed = entries
            .iter()
            .any(|existing| existing.slot_name == root_entry.slot_name);
        if !already_listed {
            entries.push(root_entry);
        }
    }
    entries.sort_by(|a, b| {
        b.modified_secs
            .cmp(&a.modified_secs)
            .then_with(|| a.slot_name.cmp(&b.slot_name))
    });
    entries
}

/// Generates the next available scenario-tagged slot name inside `dir` (e.g. `"trappist1_01"`).
pub fn suggest_next_slot_name_in_dir(
    dir: &Path,
    preset: Option<crate::simulation::scenarios::ScenarioPreset>,
) -> String {
    let prefix = preset.map_or("custom_system", |p| p.save_slot_slug());
    let existing: Vec<String> = list_save_slot_entries_in_dir(dir)
        .into_iter()
        .map(|entry| entry.slot_name)
        .collect();
    for idx in 1..=999_u32 {
        let candidate = format!("{prefix}_{idx:02}");
        let taken_in_entries = existing
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&candidate));
        let taken_on_disk = SAVE_FILE_EXTENSIONS
            .iter()
            .any(|ext| dir.join(format!("{candidate}.{ext}")).exists());
        if !taken_in_entries && !taken_on_disk {
            return candidate;
        }
    }
    format!("{prefix}_snapshot")
}

/// Generates the next available scenario-tagged save slot name in `saves/`.
pub fn suggest_next_slot_name(
    preset: Option<crate::simulation::scenarios::ScenarioPreset>,
) -> String {
    suggest_next_slot_name_in_dir(Path::new("saves"), preset)
}

/// Safely deletes a save file by path or slot name (refusing parent-traversal paths).
pub fn delete_save_slot_file(path_or_slot: &str) -> Result<String, String> {
    let trimmed = path_or_slot.trim();
    if trimmed.is_empty() {
        return Err("Save slot path cannot be empty".to_string());
    }
    if trimmed.contains("..") {
        return Err("Invalid save path".to_string());
    }
    let resolved = resolve_save_path_for_read(trimmed);
    let path = Path::new(&resolved);
    if !path.is_file() {
        return Err(format!("Save file '{resolved}' does not exist"));
    }
    let is_valid_save_ext = path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| {
            SAVE_FILE_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        });
    if !is_valid_save_ext {
        return Err("Refusing to delete non-save file".to_string());
    }
    if load_system_from_file(&resolved).is_err() {
        return Err("Refusing to delete file that is not a valid save snapshot".to_string());
    }
    std::fs::remove_file(path).map_err(|err| format!("Failed to delete '{resolved}': {err}"))?;
    Ok(resolved)
}

/// Scans `saves/` and the workspace root for valid save files (`.json`, `.bin`, `.postcard`)
/// and returns sorted unique slot stems (e.g. `["quicksave", "trappist1_01"]`).
pub fn list_save_slots() -> Vec<String> {
    let mut slots: Vec<String> = list_save_slot_entries()
        .into_iter()
        .map(|entry| entry.slot_name)
        .collect();
    slots.sort();
    slots.dedup();
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
