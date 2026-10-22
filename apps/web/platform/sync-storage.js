export const SYNC_STORES=Object.freeze(['sync_slots','sync_parts','sync_inbox','sync_receipts','sync_recovery']);
export const SYNC_STATE='sync.transport.v1',SYNC_VAULT_RECORD='sync.vault.v1';

// Call only inside the same strict transaction as the credential tombstone.
// These stores contain encrypted cache/receipts, never local library progress.
export async function clearSyncStorage(tx){for(const name of SYNC_STORES)await tx.request(tx.store(name).clear());await tx.delete('config',SYNC_STATE);}
