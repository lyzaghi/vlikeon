use crate::{Database,library::Library,now_ms,util::{digest,storage_error}};
use omniplay_contracts::*;
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use uuid::Uuid;

pub fn session_preferences(database:&Database,game_id:Uuid)->Result<SessionConfig> {
    Library::new(database).get(game_id)?;
    let value:Option<String>=database.connection.query_row("SELECT substr(value,1,4097) FROM preferences WHERE key=?1",[format!("session.{game_id}")],|r|r.get(0)).optional().map_err(storage_error)?;
    let config:SessionConfig=match value{Some(raw)if raw.len()<=4096=>serde_json::from_str(&raw).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Game settings require recovery"))?,Some(_)=>return Err(PlatformError::new(ErrorCode::CorruptData,"Game settings exceed their storage bound")),None=>SessionConfig::default()};
    config.validate()?;Ok(config)
}

pub fn set_session_preferences(database:&Database,game_id:Uuid,config:&SessionConfig)->Result<()> {
    Library::new(database).get(game_id)?;config.validate()?;
    database.connection.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",rusqlite::params![format!("session.{game_id}"),serde_json::to_string(config).map_err(storage_error)?]).map_err(storage_error)?;Ok(())
}

fn config_hash(config:&SessionConfig)->Result<ContentHash>{Ok(digest(&serde_json::to_vec(config).map_err(storage_error)?))}

#[derive(Default)]
pub struct Assistant {pending:HashMap<Uuid,SessionProposal>}
impl Assistant {
    pub fn advise(&mut self,database:&Database,game_id:Uuid,topic:AdviceTopic)->Result<LocalAdvice> {
        let settings=database.settings()?;
        if !settings.ai_enabled{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Enable local recommendations before requesting advice"));}
        let config=session_preferences(database,game_id)?;let mut next=config.clone();
        let vi=settings.locale=="vi";
        let(message,reason,change)=match topic {
            AdviceTopic::Audio=>{next.latency_frames=(config.latency_frames+1).min(8);(if vi{"Tăng hàng đợi âm thanh một mức có thể giúp giảm tiếng ngắt quãng, nhưng tăng độ trễ âm thanh. Thay đổi áp dụng ở phiên chơi tiếp theo; không thay đổi tốc độ giả lập."}else{"Increasing the audio queue by one level may reduce interruptions while increasing audio latency. It applies to the next session and does not change emulation speed."},"audio_queue",next.latency_frames!=config.latency_frames)},
            AdviceTopic::Battery=>{next.preset=PerformancePreset::Battery;(if vi{"Chế độ tiết kiệm giảm số khung hình được vẽ, giữ nguyên tốc độ giả lập và âm thanh. Hiệu quả pin phụ thuộc thiết bị; chưa có kết quả đo riêng cho máy này."}else{"Battery mode reduces presentation frequency while preserving emulation and audio speed. Battery benefit depends on the device; no device-specific measurement is available."},"battery_presentation",!matches!(config.preset,PerformancePreset::Battery))},
            AdviceTopic::Stutter=>(if vi{"Kiểm tra frame-time khi game đang chạy, đóng tác vụ nền nặng và thử core đã được đo trên cùng nhóm thiết bị. OmniPlay không tăng tốc giả lập hay chuyển core trong lúc chơi để che FPS thấp."}else{"Inspect frame times during gameplay, close heavy background tasks and prefer a core measured on the same device class. OmniPlay does not speed up emulation or switch cores mid-session to hide slow frames."},"stutter_guidance",false),
            AdviceTopic::Controls=>(if vi{"Kết nối tay cầm rồi nhấn một nút. Đặt tiêu điểm vào màn hình game để dùng bàn phím. Nhấn Escape để tạm dừng; chuyển cửa sổ sẽ xóa trạng thái phím đang giữ."}else{"Connect a controller and press a button. Focus the game canvas for keyboard input. Escape pauses the game; changing windows clears held keys."},"input_guidance",false),
            AdviceTopic::Saves=>(if vi{"Dùng bản lưu trong game để tiếp tục ổn định. Snapshot phụ thuộc core, phiên bản và cấu hình. Khi có nhánh xung đột, chọn tiến trình muốn tiếp tục trong lịch sử lưu; các bản khác vẫn được giữ."}else{"Prefer in-game saves for continuity. Snapshots depend on core, version and configuration. Select the progress to continue in save history when branches conflict; the other versions remain available."},"save_guidance",false),
            AdviceTopic::Privacy=>(if vi{"Thư viện và bản lưu ở trên thiết bị. Đồng bộ mã hóa là tùy chọn; ROM và BIOS không được đồng bộ. Trợ lý hiện tại dùng quy tắc cục bộ và không gửi nội dung đến mô hình bên ngoài."}else{"The library and saves stay on the device. Encrypted sync is optional; ROMs and BIOS files are not synchronized. The current assistant uses local rules and sends no content to an external model."},"privacy_guidance",false),
        };
        self.pending.retain(|_,p|p.expires_at_ms>now_ms());
        let proposal=if change {next.validate()?;if self.pending.len()>=32{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Too many pending recommendations"));}let value=SessionProposal{id:Uuid::new_v4(),game_id,expires_at_ms:now_ms()+300_000,before_hash:config_hash(&config)?,after:next,reason_code:reason.into()};self.pending.insert(value.id,value.clone());Some(value)}else{None};
        Ok(LocalAdvice{source:"local_rules_v1".into(),locale:settings.locale,message:message.into(),proposal,external_model_used:false})
    }
    pub fn accept(&mut self,database:&Database,id:Uuid)->Result<SessionConfig> {
        // Consent, compare-and-set and audit share one snapshot/commit. A second
        // broker cannot change preferences between the comparison and write.
        let tx=database.connection.unchecked_transaction().map_err(storage_error)?;
        if !database.settings()?.ai_enabled{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Local recommendations are disabled"));}
        let proposal=self.pending.get(&id).cloned().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Recommendation was not found"))?;
        if proposal.expires_at_ms<=now_ms(){return Err(PlatformError::new(ErrorCode::PermissionExpired,"Recommendation expired; request a fresh one"));}
        if config_hash(&session_preferences(database,proposal.game_id)?)?!=proposal.before_hash{return Err(PlatformError::new(ErrorCode::Conflict,"Game settings changed after this recommendation"));}
        if proposal.expires_at_ms<=now_ms(){return Err(PlatformError::new(ErrorCode::PermissionExpired,"Recommendation expired; request a fresh one"));}
        set_session_preferences(database,proposal.game_id,&proposal.after)?;
        database.record_audit("assistant.settings_accepted",Some(proposal.game_id))?;tx.commit().map_err(storage_error)?;self.pending.remove(&id);Ok(proposal.after)
    }
    pub fn dismiss(&mut self,id:Uuid){self.pending.remove(&id);}
}
