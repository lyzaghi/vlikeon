use crate::rejected;
use omniplay_contracts::Result;
use omniplay_core::updates::TrustedRoot;
use serde::Deserialize;

#[derive(Clone,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub schema_version:u32,
    pub enabled:bool,
    pub channel:String,
    pub platform:String,
    pub metadata_url:String,
    pub root_url_template:Option<String>,
    pub root:Option<TrustedRoot>,
    pub reason:Option<String>,
}

impl Channel {
    pub fn embedded()->Result<Self> {
        // A release build supplies the complete immutable channel JSON at compile time.
        // Mutable environment variables, web pages and command-line URLs never select trust.
        let raw=option_env!("OMNIPLAY_UPDATE_CHANNEL_JSON").unwrap_or(include_str!("../../../security/update-channel.json"));
        let channel:Self=serde_json::from_str(raw).map_err(|_|rejected("The embedded update channel is invalid"))?;
        if channel.schema_version!=1 || !omniplay_contracts::valid_identifier(&channel.channel) || !omniplay_contracts::valid_identifier(&channel.platform){return Err(rejected("The embedded channel identity is invalid"));}
        if channel.enabled {
            let root=channel.root.as_ref().ok_or_else(||rejected("No production update root is embedded"))?;
            root.validate(0)?;
            crate::transport::validate_url(&channel.metadata_url,root)?;
            if let Some(template)=&channel.root_url_template {
                if template.matches("{version}").count()!=1{return Err(rejected("Root URL template needs exactly one version placeholder"));}
                crate::transport::validate_url(&template.replace("{version}","1"),root)?;
            }
        }
        Ok(channel)
    }
    pub fn require_enabled(&self)->Result<&TrustedRoot> {
        if !self.enabled{return Err(rejected("This build has no configured production update channel"));}
        self.root.as_ref().ok_or_else(||rejected("No production update root is embedded"))
    }
}
