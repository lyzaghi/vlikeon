use crate::{SyncClient,client::save_configuration};
use omniplay_contracts::{ErrorCode,PlatformError,Result,SaveMetadata};
use omniplay_core::{Database,now_ms,saves::SaveStore,util::storage_error};
use omniplay_sync::{BlobHeader,transfer_files as files};
use rusqlite::params;
use serde::Serialize;
use std::{fs::File,io::{Seek,SeekFrom},sync::atomic::{AtomicBool,Ordering}};
use uuid::Uuid;

#[derive(Default,Clone,Serialize)]
pub struct RunSummary {pub uploaded:u32,pub downloaded:u32,pub imported:u32,pub conflicts_preserved:u32,pub waiting_for_game:u32,pub recovery_deferred:u32,pub cloud_recovery_pending:u32,pub cloud_recovered:u32,pub cloud_recovery_intact:u32,pub cloud_recovery_failed:u32,pub cloud_recovery_cleanup_pending:u32,pub last_sequence:u64,pub locally_removed_skipped:u32,pub transfer:Option<TransferSummary>}
#[derive(Clone,Serialize)]
pub struct TransferSummary{pub direction:String,pub completed_bytes:u64,pub total_bytes:u64}
fn transfer_progress(summary:&RunSummary,direction:&str,completed_bytes:u64,total_bytes:u64)->RunSummary{let mut value=summary.clone();value.transfer=Some(TransferSummary{direction:direction.into(),completed_bytes,total_bytes});value}

pub fn synchronize(database:&mut Database,cancel:&AtomicBool,on_progress:impl Fn(&RunSummary))->Result<RunSummary> {
    synchronize_with_store(database,cancel,on_progress,&crate::OsCredentialStore)
}
pub fn synchronize_with_store(database:&mut Database,cancel:&AtomicBool,on_progress:impl Fn(&RunSummary),store:&dyn crate::CredentialStore)->Result<RunSummary> {
    synchronize_with_store_deadline(database,cancel,on_progress,store,None)
}
pub fn synchronize_with_store_deadline(database:&mut Database,cancel:&AtomicBool,on_progress:impl Fn(&RunSummary),store:&dyn crate::CredentialStore,deadline:Option<std::time::Instant>)->Result<RunSummary> {
    check_enabled(database,cancel,store)?;
    let _transport_lease=crate::transfers::profile_lease(database)?;
    let mut client=SyncClient::open_with_store(database,store)?;
    client.set_deadline(deadline);
    check_enabled(database,cancel,store)?;client.transfer_capabilities()?;
    crate::transfers::cleanup_acknowledged(database,&client.configuration.account_id)?;
    let mut summary=RunSummary::default();
    summary.last_sequence=client.configuration.after_sequence;
    process_inbox(database,&client,&mut summary,cancel,store)?;
    if crate::recovery::process(database,&client,&mut summary,cancel,store,&on_progress)?{return Ok(summary);}
    on_progress(&summary);
    let mut uploaded_this_run=0;
    loop {
        check_enabled(database,cancel,store)?;
        let outgoing=outgoing(database,&client.configuration.account_id)?;
        if outgoing.is_empty(){break;}
        for metadata in outgoing {
            check_enabled(database,cancel,store)?;
            let baseline=summary.clone();
            crate::transfers::upload(database,&client,&metadata,cancel,store,&|direction,done,total|on_progress(&transfer_progress(&baseline,direction,done,total)))?;
            receipt(database,&client.configuration.account_id,metadata.id,metadata.id,"uploaded")?;
            summary.uploaded+=1;uploaded_this_run+=1;on_progress(&summary);
            crate::transfers::finish_upload(database,metadata.id)?;
            if uploaded_this_run>=500 {break;}
        }
        if uploaded_this_run>=500{break;}
    }
    for _ in 0..20 {
        check_enabled(database,cancel,store)?;
        let page=client.list(client.configuration.after_sequence,100)?;
        check_enabled(database,cancel,store)?;
        if page.is_empty(){break;}
        for object in &page {
            check_enabled(database,cancel,store)?;
            if object.sequence<=client.configuration.after_sequence{return Err(PlatformError::new(ErrorCode::CorruptData,"Synchronization cursor moved backwards"));}
            let known:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM sync_receipts WHERE account_id=?1 AND remote_object_id=?2)",params![client.configuration.account_id,object.object_id.to_string()],|row|row.get(0)).map_err(storage_error)?;
            let staged:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM sync_inbox WHERE account_id=?1 AND object_id=?2)",params![client.configuration.account_id,object.object_id.to_string()],|row|row.get(0)).map_err(storage_error)?;
            if !known&&!staged&&!omniplay_core::save_maintenance::locally_removed(database,object.object_id)? {
                let baseline=summary.clone();
                let mut encrypted=crate::transfers::download(database,&client,object.object_id,cancel,store,&|direction,done,total|on_progress(&transfer_progress(&baseline,direction,done,total)))?;
                check_enabled(database,cancel,store)?;
                stage_inbox(database,&client.configuration.account_id,object.sequence,&encrypted.header,&mut encrypted.file,&||check_enabled(database,cancel,store))?;
                summary.downloaded+=1;on_progress(&summary);
                crate::transfers::finish_download(database,encrypted)?;
            }
            client.configuration.after_sequence=object.sequence;
            // Advance only after the encrypted object is durably staged or has a
            // recorded receipt. Missing local ROMs cannot make saves disappear.
            save_configuration(database,&client.configuration)?;
            summary.last_sequence=object.sequence;
            on_progress(&summary);
        }
        process_inbox(database,&client,&mut summary,cancel,store)?;
        on_progress(&summary);
        if page.len()<100{break;}
    }
    Ok(summary)
}

fn check_enabled(database:&Database,cancel:&AtomicBool,store:&dyn crate::CredentialStore)->Result<()> {
    if cancel.load(Ordering::Acquire)||!database.settings()?.cloud_sync_opt_in{return Err(PlatformError::new(ErrorCode::Cancelled,"Synchronization stopped; existing local saves remain available"));}
    store.check_access()
}

fn outgoing(database:&Database,account:&str)->Result<Vec<SaveMetadata>> {
    let pending=crate::transfers::pending_upload_id(database,account)?.map(|id|id.to_string());
    let mut query=database.connection.prepare("SELECT metadata_json FROM saves s WHERE NOT EXISTS(SELECT 1 FROM sync_receipts r WHERE r.account_id=?1 AND r.save_id=s.id) ORDER BY CASE WHEN s.id=?2 THEN 0 ELSE 1 END,s.created_at_ms,s.generation,s.id LIMIT 100").map_err(storage_error)?;
    let records=query.query_map(params![account,pending],|row|row.get::<_,String>(0)).map_err(storage_error)?;
    let projected=records.map(|record|serde_json::from_str::<SaveMetadata>(&record.map_err(storage_error)?).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"A local save record is invalid"))).collect::<Result<Vec<_>>>()?;
    drop(query);
    projected.into_iter().map(|metadata|omniplay_core::save_origins::for_sync(database,metadata.id).map(|(canonical,_)|canonical)).collect()
}

fn receipt(database:&Database,account:&str,save:Uuid,remote:Uuid,direction:&str)->Result<()> {
    database.connection.execute("INSERT INTO sync_receipts(account_id,save_id,remote_object_id,direction,acknowledged_at_ms) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(account_id,save_id) DO NOTHING",params![account,save.to_string(),remote.to_string(),direction,now_ms()]).map_err(storage_error)?;
    Ok(())
}

fn stage_inbox(database:&Database,account:&str,sequence:u64,header:&BlobHeader,source:&mut File,check:&dyn Fn()->Result<()>)->Result<()> {
    header.validate()?;
    if header.binding.account_id!=account||sequence>i64::MAX as u64{return Err(PlatformError::new(ErrorCode::CorruptData,"Remote object account or sequence is invalid"));}
    check()?;
    let root=files::private_dir(&database.root,"sync-inbox").map_err(storage_error)?;
    let directory=files::private_dir(&root,account).map_err(storage_error)?;
    let target=directory.join(format!("{}.blob",header.binding.object_id));
    match files::open(&target,false,header.ciphertext_bytes){
        Ok(mut existing)=>header.copy_blob_checked(&mut existing,&mut std::io::sink(),check)?,
        Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{
            source.seek(SeekFrom::Start(0)).map_err(storage_error)?;
            let mut pending=files::Pending::create(&directory).map_err(storage_error)?;
            header.copy_blob_checked(source,pending.file().map_err(storage_error)?,check)?;check()?;
            pending.publish(&target).map_err(storage_error)?;
        },
        Err(error)=>return Err(storage_error(error)),
    }
    check()?;
    database.connection.execute("INSERT INTO sync_inbox(account_id,object_id,remote_sequence,header_json,received_at_ms) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(account_id,object_id) DO NOTHING",params![account,header.binding.object_id.to_string(),sequence,serde_json::to_string(&header).map_err(storage_error)?,now_ms()]).map_err(storage_error)?;
    Ok(())
}

fn process_inbox(database:&mut Database,client:&SyncClient,summary:&mut RunSummary,cancel:&AtomicBool,store:&dyn crate::CredentialStore)->Result<()> {
    let account=&client.configuration.account_id;
    let records={
        let mut query=database.connection.prepare("SELECT object_id,header_json FROM sync_inbox WHERE account_id=?1 ORDER BY remote_sequence LIMIT 2000").map_err(storage_error)?;
        let rows=query.query_map([account],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(storage_error)?;
        rows.collect::<std::result::Result<Vec<_>,_>>().map_err(storage_error)?
    };
    summary.waiting_for_game=0;summary.recovery_deferred=0;
    for (id,json) in records {
        check_enabled(database,cancel,store)?;
        let id=Uuid::parse_str(&id).map_err(storage_error)?;
        let header:BlobHeader=serde_json::from_str(&json).map_err(storage_error)?;
        header.validate()?;
        if header.binding.account_id!=*account||header.binding.object_id!=id{return Err(PlatformError::new(ErrorCode::CorruptData,"Local sync inbox identity is invalid"));}
        let path=database.root.join("sync-inbox").join(account).join(format!("{id}.blob"));
        if omniplay_core::save_maintenance::locally_removed(database,id)? {
            database.connection.execute("DELETE FROM sync_inbox WHERE account_id=?1 AND object_id=?2",params![account,id.to_string()]).map_err(storage_error)?;
            let _=std::fs::remove_file(&path);summary.locally_removed_skipped+=1;continue;
        }
        let mut file=files::open(&path,false,header.ciphertext_bytes).map_err(storage_error)?;
        let (metadata,payload)=client.cipher.decrypt_blob_reader_checked(&header,&mut file,&||check_enabled(database,cancel,store))?;
        check_enabled(database,cancel,store)?;
        let imported=match SaveStore::new(database).ingest_remote(metadata,&payload){
            Ok(value)=>value,
            Err(error)if error.code==ErrorCode::Busy=>{drop(payload);summary.recovery_deferred+=1;continue;},
            Err(error)=>return Err(error),
        };drop(payload);
        if let Some(saved)=imported {
            receipt(database,account,saved.id,id,"downloaded")?;
            database.connection.execute("DELETE FROM sync_inbox WHERE account_id=?1 AND object_id=?2",params![account,id.to_string()]).map_err(storage_error)?;
            drop(file);
            let _=std::fs::remove_file(path);
            summary.imported+=1;if saved.conflict{summary.conflicts_preserved+=1;}
        }else if omniplay_core::save_maintenance::locally_removed(database,id)? {
            database.connection.execute("DELETE FROM sync_inbox WHERE account_id=?1 AND object_id=?2",params![account,id.to_string()]).map_err(storage_error)?;
            drop(file);let _=std::fs::remove_file(path);summary.locally_removed_skipped+=1;
        }else {summary.waiting_for_game+=1;}
    }
    Ok(())
}
