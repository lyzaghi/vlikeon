import Foundation
import UIKit

enum DeviceSnapshot {
    // Invoked on the main queue, after an authenticated explicit device request.
    static func augment(_ response:[String:Any])->[String:Any] {
        guard Thread.isMainThread,response["status"] as? String == "ok",var report=response["result"] as? [String:Any],report["schema_version"] as? Int == 1 else{return response}
        let process=ProcessInfo.processInfo
        var power:[String:Any]=["battery_present":NSNull(),"ac_connected":NSNull(),"charging":NSNull(),"battery_percent":NSNull(),"battery_saver":process.isLowPowerModeEnabled]
        let device=UIDevice.current,wasMonitoring=device.isBatteryMonitoringEnabled
        if !wasMonitoring {device.isBatteryMonitoringEnabled=true}
        defer{if !wasMonitoring {device.isBatteryMonitoringEnabled=false}}
        let level=device.batteryLevel,state=device.batteryState
        if level.isFinite,level>=0,level<=1 {power["battery_present"]=true;power["battery_percent"]=Int((level*100).rounded())}
        switch state {
        case .charging:power["battery_present"]=true;power["ac_connected"]=true;power["charging"]=true
        case .full:power["battery_present"]=true;power["ac_connected"]=true;power["charging"]=false
        case .unplugged:power["battery_present"]=true;power["ac_connected"]=false;power["charging"]=false
        case .unknown:break
        @unknown default:break
        }
        var thermal:Any=NSNull()
        switch process.thermalState {case .nominal:thermal="nominal";case .fair:thermal="fair";case .serious:thermal="serious";case .critical:thermal="critical";@unknown default:break}
        let version=process.operatingSystemVersion
        report["platform_source"]="apple_foundation_uikit";report["platform_version"]="\(version.majorVersion).\(version.minorVersion).\(version.patchVersion)";report["platform_sampled_at_ms"]=Int64(Date().timeIntervalSince1970*1000);report["thermal_state"]=thermal;report["power"]=power
        if process.physicalMemory>0,process.physicalMemory<=9_007_199_254_740_991 {report["total_memory_bytes"]=process.physicalMemory}
        var result=response;result["result"]=report;return result
    }
}
