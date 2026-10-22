use serde::{Deserialize,Serialize};

/// Profile-local staging reservations, not a measurement of allocated disk.
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeWorkStorageStatus {
    pub schema_version:u32,
    pub works:u32,
    pub reserved_works:u32,
    pub staged_works:u32,
    pub retiring_works:u32,
    pub reserved_bytes:u64,
    pub reserved_files:u32,
    pub maximum_works:u32,
    pub maximum_bytes:u64,
    pub maximum_files:u32,
    pub physical_bytes:Option<u64>,
    pub recovery_running:bool,
}

#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeWorkRecoverySummary {
    pub attempted:u32,
    pub recovered:u32,
    pub retained:u32,
    pub unknown_entries:u32,
    pub cancelled:bool,
    pub more_available:bool,
}
