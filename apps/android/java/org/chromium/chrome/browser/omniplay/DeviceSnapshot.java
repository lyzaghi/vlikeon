package org.chromium.chrome.browser.omniplay;

import android.app.ActivityManager;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;
import android.os.Build;
import android.os.PowerManager;
import org.json.JSONObject;

/** Only augments an explicitly requested, authenticated local device snapshot. */
final class DeviceSnapshot {
    private static String safe(String value){if(value==null)return "";StringBuilder out=new StringBuilder();for(int at=0;at<value.length()&&out.length()<96;at++){char c=value.charAt(at);if(!Character.isISOControl(c)&&!(c>=0x202a&&c<=0x202e)&&!(c>=0x2066&&c<=0x2069))out.append(c);}return out.toString();}
    static JSONObject augment(Context context,JSONObject response){
        if(!"ok".equals(response.optString("status")))return response;
        JSONObject report=response.optJSONObject("result");if(report==null||report.optInt("schema_version")!=1)return response;
        try{
            // Unsupported/private kernel files are never replaced with guesses.
            JSONObject power=new JSONObject().put("battery_present",JSONObject.NULL).put("ac_connected",JSONObject.NULL).put("charging",JSONObject.NULL).put("battery_percent",JSONObject.NULL).put("battery_saver",JSONObject.NULL);
            report.put("power",power).put("platform_version",safe(Build.VERSION.RELEASE)+" / API "+Build.VERSION.SDK_INT).put("platform_source","android_framework").put("platform_sampled_at_ms",System.currentTimeMillis()).put("thermal_state",JSONObject.NULL);
            try{ActivityManager manager=context.getSystemService(ActivityManager.class);if(manager!=null){ActivityManager.MemoryInfo memory=new ActivityManager.MemoryInfo();manager.getMemoryInfo(memory);if(memory.totalMem>0&&memory.totalMem<=9007199254740991L)report.put("total_memory_bytes",memory.totalMem);if(memory.availMem>=0&&memory.availMem<=memory.totalMem)report.put("available_memory_bytes",memory.availMem);}}catch(RuntimeException ignored){}
            try{
                Intent battery=context.registerReceiver(null,new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
                if(battery!=null){
                    if(battery.hasExtra(BatteryManager.EXTRA_PRESENT))power.put("battery_present",battery.getBooleanExtra(BatteryManager.EXTRA_PRESENT,false));
                    int plugged=battery.getIntExtra(BatteryManager.EXTRA_PLUGGED,-1);if(plugged>=0)power.put("ac_connected",plugged!=0);
                    if(!power.has("battery_present")||power.isNull("battery_present")||power.optBoolean("battery_present")){
                        int level=battery.getIntExtra(BatteryManager.EXTRA_LEVEL,-1),scale=battery.getIntExtra(BatteryManager.EXTRA_SCALE,-1);if(level>=0&&scale>0&&level<=scale)power.put("battery_percent",Math.round((double)level*100/scale));
                        int state=battery.getIntExtra(BatteryManager.EXTRA_STATUS,BatteryManager.BATTERY_STATUS_UNKNOWN);if(state==BatteryManager.BATTERY_STATUS_CHARGING)power.put("charging",true);else if(state==BatteryManager.BATTERY_STATUS_DISCHARGING||state==BatteryManager.BATTERY_STATUS_NOT_CHARGING||state==BatteryManager.BATTERY_STATUS_FULL)power.put("charging",false);
                    }
                }
            }catch(RuntimeException ignored){}
            try{
                PowerManager manager=context.getSystemService(PowerManager.class);if(manager!=null){power.put("battery_saver",manager.isPowerSaveMode());if(Build.VERSION.SDK_INT>=29){String thermal=null;switch(manager.getCurrentThermalStatus()){case PowerManager.THERMAL_STATUS_NONE:thermal="none";break;case PowerManager.THERMAL_STATUS_LIGHT:thermal="light";break;case PowerManager.THERMAL_STATUS_MODERATE:thermal="moderate";break;case PowerManager.THERMAL_STATUS_SEVERE:thermal="severe";break;case PowerManager.THERMAL_STATUS_CRITICAL:thermal="critical";break;case PowerManager.THERMAL_STATUS_EMERGENCY:thermal="emergency";break;case PowerManager.THERMAL_STATUS_SHUTDOWN:thermal="shutdown";break;default:break;}report.put("thermal_state",thermal==null?JSONObject.NULL:thermal);}}
            }catch(RuntimeException ignored){}
        }catch(org.json.JSONException ignored){}
        return response;
    }
}
