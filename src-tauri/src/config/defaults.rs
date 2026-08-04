//! Default values for every setting. Kept in one place so the UI, the
//! engine and the migration code agree.

pub const SCHEMA_VERSION: u32 = 1;
pub const INTERVAL_SECS: u64 = 60;
pub const PAUSE_WHEN_INPUT_RECENT: bool = true;
pub const END_OF_DAY: &str = "18:00";
pub const HOTKEY: &str = "CmdOrCtrl+Alt+E";

pub fn suggested_process_names() -> Vec<String> {
    ["Teams", "teams", "msedge", "chrome"]
        .map(String::from)
        .to_vec()
}
