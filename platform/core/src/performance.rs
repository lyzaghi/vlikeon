use crate::{Database,library::Library,now_ms,util::{digest,storage_error}};
use omniplay_contracts::*;
use rusqlite::{params,OptionalExtension};
use std::io::Write;
use uuid::Uuid;

pub struct PerformanceRuns<'a>{database:&'a mut Database}
impl<'a> PerformanceRuns<'a> {
    pub fn new(database:&'a mut Database)->Self{Self{database}}
    pub fn commit(&mut self,input:PerformanceRunInput)->Result<PerformanceRunSummary> {
        self.commit_from(input,"web_worker_performance_clock")
    }
    // Only the trusted broker's contained-runtime path calls this entry point.
    // The renderer command never accepts a measurement-source selector.
    pub fn commit_native(&mut self,input:PerformanceRunInput)->Result<PerformanceRunSummary>{
        self.commit_from(input,"native_frontend_steady_clock")
    }
    fn commit_from(&mut self,input:PerformanceRunInput,source:&str)->Result<PerformanceRunSummary> {
        let game=Library::new(self.database).get(input.game_id)?;
        let native=source=="native_frontend_steady_clock";
        let minimum_clock=if native{1.0}else{20.0};
        let valid=input.game_content_hash==game.content_hash&&valid_identifier(&input.runtime_id)&&input.source_revision.len()==40&&input.source_revision.bytes().all(|v|v.is_ascii_hexdigit()&&!v.is_ascii_uppercase())&&(120_000..=180_000).contains(&input.observed_duration_ms)&&input.warmup_ms>=10_000&&input.warmup_ms<=60_000&&input.nominal_refresh_hz.is_finite()&&(minimum_clock..=240.0).contains(&input.nominal_refresh_hz)&&(100..=36_000).contains(&input.frame_intervals_ms.len())&&input.frame_intervals_ms.iter().all(|v|v.is_finite()&&*v>0.0&&*v<=60_000.0)&&(!native||input.frames_emulated as usize==input.frame_intervals_ms.len());
        if !valid{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Performance run is incomplete or outside its measurement bounds"));}
        ContentHash::parse(input.runtime_build_hash.0.clone())?;ContentHash::parse(input.core_config_hash.0.clone())?;
        let raw=serde_json::to_vec(&input).map_err(storage_error)?;
        if raw.len()>768*1024{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Performance samples exceed their storage bound"));}
        let hash=digest(&raw);
        let existing:Option<(String,String)>=self.database.connection.query_row("SELECT input_hash,summary_json FROM performance_runs WHERE id=?1",[input.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage_error)?;
        if let Some((previous,summary))=existing {
            let summary:PerformanceRunSummary=serde_json::from_str(&summary).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Performance summary is invalid"))?;
            if previous!=hash.0||summary.measurement_source!=source{return Err(PlatformError::new(ErrorCode::Conflict,"Performance run identity was reused with different measurements or origin"));}return Ok(summary);
        }
        let total=input.frame_intervals_ms.iter().sum::<f64>();
        if (total-f64::from(input.observed_duration_ms)).abs()>3000.0||input.frames_emulated<100||input.frames_presented>input.frames_emulated.saturating_mul(4){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Performance samples do not cover the observed duration"));}
        let mut sorted=input.frame_intervals_ms.clone();sorted.sort_by(f64::total_cmp);
        let quantile=|q:f64|sorted[((sorted.len()-1)as f64*q).floor()as usize];
        let seconds=f64::from(input.observed_duration_ms)/1000.0;
        let summary=PerformanceRunSummary{id:input.id,game_id:input.game_id,created_at_ms:now_ms(),platform:format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),device_class:"unclassified".into(),runtime_id:input.runtime_id.clone(),runtime_version:input.runtime_version.clone(),source_revision:input.source_revision.clone(),runtime_build_hash:input.runtime_build_hash.clone(),game_content_hash:input.game_content_hash.clone(),core_config_hash:input.core_config_hash.clone(),preset:input.preset,observed_duration_ms:input.observed_duration_ms,warmup_ms:input.warmup_ms,nominal_refresh_hz:input.nominal_refresh_hz,frames_presented:input.frames_presented,frames_emulated:input.frames_emulated,dropped_frames:input.dropped_frames,audio_underruns:input.audio_underruns,frame_time_p50_ms:quantile(0.50),frame_time_p95_ms:quantile(0.95),frame_time_p99_ms:quantile(0.99),frames_per_second:f64::from(input.frames_emulated)/seconds,presented_frames_per_second:f64::from(input.frames_presented)/seconds,measurement_source:source.into(),independently_attested:false};
        let tx=self.database.connection.transaction().map_err(storage_error)?;
        tx.execute("INSERT INTO performance_runs(id,game_id,created_at_ms,input_hash,input_json,summary_json) VALUES(?1,?2,?3,?4,?5,?6)",params![input.id.to_string(),input.game_id.to_string(),summary.created_at_ms,hash.0,String::from_utf8(raw).map_err(storage_error)?,serde_json::to_string(&summary).map_err(storage_error)?]).map_err(storage_error)?;
        tx.execute("DELETE FROM performance_runs WHERE id NOT IN (SELECT id FROM performance_runs ORDER BY created_at_ms DESC,id DESC LIMIT 20)",[]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;Ok(summary)
    }
    pub fn list(&self,game_id:Uuid)->Result<Vec<PerformanceRunSummary>> {
        let mut query=self.database.connection.prepare("SELECT summary_json FROM performance_runs WHERE game_id=?1 ORDER BY created_at_ms DESC LIMIT 20").map_err(storage_error)?;
        let rows=query.query_map([game_id.to_string()],|r|r.get::<_,String>(0)).map_err(storage_error)?;
        rows.map(|r|serde_json::from_str(&r.map_err(storage_error)?).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Performance summary is invalid"))).collect()
    }
    fn summary(&self,id:Uuid)->Result<PerformanceRunSummary>{let raw:String=self.database.connection.query_row("SELECT summary_json FROM performance_runs WHERE id=?1",[id.to_string()],|r|r.get(0)).optional().map_err(storage_error)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Performance run was not found"))?;serde_json::from_str(&raw).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Performance summary is invalid"))}
    pub fn compare(&self,baseline:Uuid,candidate:Uuid)->Result<PerformanceComparison> {
        let baseline=self.summary(baseline)?;let candidate=self.summary(candidate)?;
        if baseline.game_content_hash!=candidate.game_content_hash||baseline.runtime_id!=candidate.runtime_id||baseline.platform!=candidate.platform||baseline.nominal_refresh_hz!=candidate.nominal_refresh_hz||baseline.measurement_source!=candidate.measurement_source{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Compare the same content, runtime, nominal clock, measurement source and platform"));}
        Ok(PerformanceComparison{frame_time_p95_change_percent:(candidate.frame_time_p95_ms/baseline.frame_time_p95_ms-1.0)*100.0,audio_underrun_change:i64::from(candidate.audio_underruns)-i64::from(baseline.audio_underruns),baseline,candidate,controlled_hardware_comparison:false})
    }
    pub fn export(&self,id:Uuid,destination:&mut std::fs::File)->Result<()> {
        let(summary,input):(String,String)=self.database.connection.query_row("SELECT summary_json,input_json FROM performance_runs WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage_error)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Performance run was not found"))?;
        let data=serde_json::json!({"schema_version":1,"summary":serde_json::from_str::<serde_json::Value>(&summary).map_err(storage_error)?,"measurements":serde_json::from_str::<serde_json::Value>(&input).map_err(storage_error)?});
        destination.write_all(&serde_json::to_vec_pretty(&data).map_err(storage_error)?).map_err(storage_error)?;destination.sync_all().map_err(storage_error)
    }
    pub fn delete(&mut self,id:Uuid)->Result<()> {self.database.connection.execute("DELETE FROM performance_runs WHERE id=?1",[id.to_string()]).map_err(storage_error)?;Ok(())}
    pub fn clear(&mut self)->Result<()> {self.database.connection.execute("DELETE FROM performance_runs",[]).map_err(storage_error)?;Ok(())}
}
