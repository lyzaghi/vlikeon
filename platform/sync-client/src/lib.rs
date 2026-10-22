#![forbid(unsafe_code)]
mod client;
mod reconciliation;
mod vault;
mod transfers;
mod recovery;

pub use client::{SyncClient,SyncConfiguration,CreatedAccount,load_configuration,create_account,register_account,recover_account,renew_certificate,revoke_device,delete_account,disconnect};
pub use reconciliation::{RunSummary,TransferSummary,synchronize};
pub use vault::{CredentialStore,OsCredentialStore,MAX_CREDENTIAL_BYTES};
pub use client::{create_account_with_store,register_account_with_store,recover_account_with_store,renew_certificate_with_store,revoke_device_with_store,delete_account_with_store,disconnect_with_store};
pub use reconciliation::{synchronize_with_store,synchronize_with_store_deadline};
pub use recovery::{request as request_save_recovery,list as list_save_recoveries,cancel as cancel_save_recovery};
