//! Retained encoded parameter versions, not stream/decoder authorization. Only
//! this table's parsers create stored values; callers receive immutable borrows.
use crate::{invalid,native_h264_bits::ParameterSetBits,native_h264_sps::{Sps,PictureOrder},native_h264_pps::{Pps,SliceGroups}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{collections::BTreeMap,sync::Arc};

const MAX_ENCODED:usize=1024*1024;
const MAX_MAP_BYTES:usize=1024*1024;
const MAX_CHARGED:usize=2*1024*1024;
const MAX_VERSION:u64=10_000_000;
#[derive(Clone,Copy,Default)]
struct Cost{encoded:usize,maps:usize,charged:usize}
impl Cost{
    fn plus(self,other:Self)->Result<Self>{
        let value=Self{encoded:self.encoded.checked_add(other.encoded).ok_or_else(resource)?,
            maps:self.maps.checked_add(other.maps).ok_or_else(resource)?,charged:self.charged.checked_add(other.charged).ok_or_else(resource)?};
        if value.encoded>MAX_ENCODED||value.maps>MAX_MAP_BYTES||value.charged>MAX_CHARGED{return Err(resource());}Ok(value)
    }
}
struct StoredSps{version:u64,encoded:Vec<u8>,value:Sps,cost:Cost}
struct StoredPps{version:u64,sps_version:u64,encoded:Vec<u8>,value:Pps,cost:Cost}
/// Table-local version binding. Not serializable, not a decoder or media permit.
#[derive(Clone,Debug)]
pub struct ParameterBinding{table:Arc<()>,sps:u8,sps_version:u64,pps:u8,pps_version:u64}
impl PartialEq for ParameterBinding{
    fn eq(&self,other:&Self)->bool{Arc::ptr_eq(&self.table,&other.table)&&self.sps==other.sps&&self.sps_version==other.sps_version&&self.pps==other.pps&&self.pps_version==other.pps_version}
}
impl Eq for ParameterBinding{}
pub struct ResolvedParameters<'a>{table:&'a Arc<()>,sps:&'a StoredSps,pps:&'a StoredPps}
impl ResolvedParameters<'_>{
    pub fn sps(&self)->&Sps{&self.sps.value}
    pub fn pps(&self)->&Pps{&self.pps.value}
    pub fn encoded_sps(&self)->&[u8]{&self.sps.encoded}
    pub fn encoded_pps(&self)->&[u8]{&self.pps.encoded}
    pub fn binding(&self)->ParameterBinding{ParameterBinding{table:Arc::clone(self.table),sps:self.sps.value.id,sps_version:self.sps.version,pps:self.pps.value.id,pps_version:self.pps.version}}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ParameterUpdate{pub id:u8,pub version:u64,pub changed:bool,pub invalidated_pps:usize}
pub struct ParameterSets{identity:Arc<()>,sps:BTreeMap<u8,StoredSps>,pps:BTreeMap<u8,StoredPps>,version:u64,closed:bool}
impl Default for ParameterSets{fn default()->Self{Self::new()}}
impl ParameterSets{
    pub fn new()->Self{Self{identity:Arc::new(()),sps:BTreeMap::new(),pps:BTreeMap::new(),version:0,closed:false}}
    pub fn close(&mut self){self.closed=true;self.pps.clear();self.sps.clear();}
    pub fn version(&self)->u64{self.version}
    fn live(&self)->Result<()>{if self.closed{Err(invalid())}else{Ok(())}}
    fn next(&self)->Result<u64>{self.live()?;self.version.checked_add(1).filter(|v|*v<=MAX_VERSION).ok_or_else(resource)}
    /// Replacing SPS ID invalidates every retained PPS tied to that ID. Even
    /// identical PPS bytes must be resent/reparsed against the new SPS version.
    /// Returned errors do not replace parameter state or advance the version.
    pub fn install_sps(&mut self,nal:&[u8])->Result<ParameterUpdate>{
        self.live()?;let value=Sps::parse(nal)?;let id=value.id;
        if let Some(old)=self.sps.get(&id){if old.encoded.as_slice()==nal{return Ok(ParameterUpdate{id,version:old.version,changed:false,invalidated_pps:0});}}
        let version=self.next()?;let cost=sps_cost(nal.len(),&value);
        let mut total=cost;let mut invalidated=0;
        // Preflight aggregate quotas before invalidating any old dependencies.
        total=Cost::default().plus(total)?;
        for (key,entry) in &self.sps{if *key!=id{total=total.plus(entry.cost)?;}}
        for entry in self.pps.values(){if entry.value.sps_id==id{invalidated+=1;}else{total=total.plus(entry.cost)?;}}
        let encoded=copy_nal(nal)?;
        self.pps.retain(|_,entry|entry.value.sps_id!=id);
        self.sps.insert(id,StoredSps{version,encoded,value,cost});self.version=version;
        Ok(ParameterUpdate{id,version,changed:true,invalidated_pps:invalidated})
    }
    pub fn install_pps(&mut self,nal:&[u8])->Result<ParameterUpdate>{
        self.live()?;
        let sps_id={let mut bits=ParameterSetBits::from_nal(nal)?;if bits.nal_type()!=8{return Err(invalid());}bits.ue(255)?;bits.ue(31)? as u8};
        let sps=self.sps.get(&sps_id).ok_or_else(invalid)?;
        let value=Pps::parse(nal,&sps.value)?;let id=value.id;let sps_version=sps.version;
        if let Some(old)=self.pps.get(&id){if old.sps_version==sps_version&&old.encoded.as_slice()==nal{return Ok(ParameterUpdate{id,version:old.version,changed:false,invalidated_pps:0});}}
        let version=self.next()?;let cost=pps_cost(nal.len(),&value);
        let mut total=Cost::default().plus(cost)?;
        for entry in self.sps.values(){total=total.plus(entry.cost)?;}
        for (key,entry) in &self.pps{if *key!=id{total=total.plus(entry.cost)?;}}
        let encoded=copy_nal(nal)?;
        self.pps.insert(id,StoredPps{version,sps_version,encoded,value,cost});self.version=version;
        Ok(ParameterUpdate{id,version,changed:true,invalidated_pps:0})
    }
    /// Only resolves an actual stored, parsed, version-matched dependency pair.
    /// It says nothing about native owner, negotiation, slices or decodability.
    pub fn resolve(&self,pps_id:u8)->Result<ResolvedParameters<'_>>{
        self.live()?;let pps=self.pps.get(&pps_id).ok_or_else(invalid)?;
        let sps=self.sps.get(&pps.value.sps_id).ok_or_else(invalid)?;
        if pps.sps_version!=sps.version{return Err(invalid());}
        Ok(ResolvedParameters{table:&self.identity,sps,pps})
    }
    pub fn check(&self,binding:&ParameterBinding)->Result<ResolvedParameters<'_>>{
        if !Arc::ptr_eq(&self.identity,&binding.table){return Err(invalid());}
        let value=self.resolve(binding.pps)?;
        if value.sps.value.id!=binding.sps||value.sps.version!=binding.sps_version||value.pps.version!=binding.pps_version{return Err(invalid());}Ok(value)
    }
}
fn resource()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"The native H.264 parameter-version budget is unavailable")}
fn copy_nal(nal:&[u8])->Result<Vec<u8>>{let mut value=Vec::new();value.try_reserve_exact(nal.len()).map_err(|_|resource())?;value.extend_from_slice(nal);Ok(value)}
fn sps_cost(encoded:usize,value:&Sps)->Cost{
    let offsets=match &value.picture_order{PictureOrder::Cycle{offsets,..}=>offsets.len()*4,_=>0};
    let scaling:usize=value.scaling_lists.iter().flatten().map(Vec::len).sum();
    let cpb=value.vui.as_ref().map(|v|v.nal_hrd.as_ref().map(|h|h.cpb.len()).unwrap_or(0)+v.vcl_hrd.as_ref().map(|h|h.cpb.len()).unwrap_or(0)).unwrap_or(0);
    Cost{encoded,maps:0,charged:4096+encoded+offsets+scaling+cpb*32}
}
fn pps_cost(encoded:usize,value:&Pps)->Cost{
    let maps=match &value.slice_groups{Some(SliceGroups::Explicit{ids,..})=>ids.len(),_=>0};
    let scaling:usize=value.scaling_lists.iter().flatten().map(Vec::len).sum();
    Cost{encoded,maps,charged:4096+encoded+maps+scaling}
}
