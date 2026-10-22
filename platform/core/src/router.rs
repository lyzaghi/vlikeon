use omniplay_contracts::*;

pub struct RoutingPolicy {pub native_allowed:bool,pub remote_allowed:bool,pub paired_host_available:bool,pub device_class:String}

pub fn rank(game:&GameManifest,device:&DeviceCapabilities,candidates:&[RuntimeCandidate],policy:&RoutingPolicy,now:u64)->Result<RouteDecision> {
    device.validate()?;game.validate()?;
    if candidates.len()>32||policy.device_class.len()>128{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Runtime routing input exceeds its bound"));}
    let mut ids=std::collections::BTreeSet::new();let mut ranked=Vec::new();
    for candidate in candidates {
        if !valid_identifier(&candidate.id)||!ids.insert(&candidate.id)||candidate.systems.len()>32{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Runtime routing identity is invalid"));}
        let mut reasons=Vec::new();let mut eligible=true;let mut score=match candidate.kind{RuntimeKind::WebWasm=>100,RuntimeKind::NativeEmulator=>110,RuntimeKind::Remote=>60,RuntimeKind::InstalledGame=>120};
        if !candidate.available{eligible=false;reasons.push(RouteReason::NotInstalled);}
        if !candidate.distribution_approved{eligible=false;reasons.push(RouteReason::Unapproved);}
        if candidate.security_revoked{eligible=false;reasons.push(RouteReason::Revoked);}
        if !candidate.systems.contains(&game.system){eligible=false;reasons.push(RouteReason::WrongSystem);}
        if candidate.kind==RuntimeKind::WebWasm&&!device.wasm{eligible=false;reasons.push(RouteReason::RequiresWebAssembly);}
        if candidate.needs_webgpu&&!device.webgpu{eligible=false;reasons.push(RouteReason::RequiresWebGpu);}
        if candidate.minimum_memory_mb.zip(device.memory_mb).is_some_and(|(needed,available)|needed>available){eligible=false;reasons.push(RouteReason::NotEnoughMemory);}
        if matches!(candidate.kind,RuntimeKind::NativeEmulator|RuntimeKind::InstalledGame)&&!policy.native_allowed{eligible=false;reasons.push(RouteReason::ConsentRequired);}
        if candidate.kind==RuntimeKind::Remote {
            if !policy.remote_allowed{eligible=false;reasons.push(RouteReason::ConsentRequired);}
            if !policy.paired_host_available{eligible=false;reasons.push(RouteReason::RemoteRequiresPairedHost);}
        }else if eligible {reasons.push(RouteReason::AvailableLocalRuntime);}
        let benchmark=candidate.benchmark.as_ref().filter(|b|{
            b.device_class==policy.device_class&&b.game_content_hash==game.content_hash&&b.source_revision.len()==40&&b.source_revision.bytes().all(|c|c.is_ascii_hexdigit())&&b.measured_at_ms<=now&&now-b.measured_at_ms<=90*24*60*60*1000&&b.sustained_seconds>=120&&b.frame_time_p95_ms.is_finite()&&b.frame_time_p95_ms>0.0&&b.nominal_refresh_hz.is_finite()&&(20.0..=240.0).contains(&b.nominal_refresh_hz)
        });
        match benchmark {
            Some(b)if b.frame_time_p95_ms<=1000.0/b.nominal_refresh_hz*1.10&&b.audio_underruns==0=>{score+=50;reasons.push(RouteReason::MeasuredFrameBudgetMet);}
            Some(_)=>{score-=70;reasons.push(RouteReason::MeasuredFrameBudgetExceeded);}
            None=>reasons.push(RouteReason::UnmeasuredOnDevice),
        }
        if device.battery_percent.is_some_and(|v|v<20.0){if candidate.kind==RuntimeKind::Remote{score+=10;}reasons.push(RouteReason::BatteryConservation);}
        if device.thermal_throttled==Some(true){if candidate.kind==RuntimeKind::Remote{score+=25;}reasons.push(RouteReason::ThermalPressure);}
        ranked.push(RankedRuntime{id:candidate.id.clone(),kind:candidate.kind,eligible,score,reasons});
    }
    ranked.sort_by(|a,b|b.eligible.cmp(&a.eligible).then_with(||b.score.cmp(&a.score)).then_with(||a.id.cmp(&b.id)));
    Ok(RouteDecision{recommended_id:ranked.iter().find(|r|r.eligible).map(|r|r.id.clone()),candidates:ranked,automatic_mid_session_switch:false})
}
