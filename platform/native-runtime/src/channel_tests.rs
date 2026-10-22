// Authored for item 16. These exercise the production codec and admission latch;
// the process counter below is not evidence of actual OS/process termination.
use super::*;
use serde_json::json;
use std::{io::Cursor,sync::atomic::AtomicUsize};

fn framed(header:&[u8],declared:u32,payload:&[u8])->Vec<u8>{let mut bytes=(header.len()as u32).to_le_bytes().to_vec();bytes.extend_from_slice(header);bytes.extend_from_slice(&declared.to_le_bytes());bytes.extend_from_slice(payload);bytes}
fn request(method:&str,params:Value)->Request{Request{protocol:1,id:1,method:method.into(),params}}

#[test]
fn malformed_envelopes_do_not_read_a_body_length(){
    let cases:[&[u8];12]=[
        br#"{"protocol":1,"id":1,"status":"bogus","error_code":"storage_failure"}"#,
        br#"{"protocol":1,"id":2,"status":"ok","result":{}}"#,
        br#"{"protocol":true,"id":1,"status":"ok","result":{}}"#,
        br#"{"protocol":1,"id":1,"status":"ok","result":null}"#,
        br#"{"protocol":1,"id":1,"status":"ok","result":{},"error_code":null}"#,
        br#"{"protocol":1,"id":1,"status":"error","error_code":"storage_failure","result":null}"#,
        br#"{"protocol":1,"id":1,"status":"error","error_code":"unknown_child_text"}"#,
        br#"{"protocol":1,"id":1,"status":"error","error_code":null}"#,
        br#"{"protocol":1,"id":1,"status":"ok"}"#,
        br#"{"protocol":1,"id":1,"status":"error"}"#,
        br#"{"protocol":1,"protocol":1,"id":1,"status":"ok","result":{}}"#,
        br#"{"protocol":1,"id":1,"status":"ok","status":"ok","result":{}}"#,
    ];
    for header in cases{let mut stream=Cursor::new(framed(header,u32::MAX,&[]));let error=decode_response(&mut stream,1,"save").err().expect("reject malformed envelope");assert_eq!(error.code,ErrorCode::RuntimeCrashed);assert_eq!(stream.position(),4+header.len()as u64,"must not read even the binary length");}
}
#[test]
fn only_fully_framed_empty_error_payload_is_recoverable(){
    let header=br#"{"protocol":1,"id":1,"status":"error","error_code":"storage_failure"}"#;
    let error=decode_response(&mut Cursor::new(framed(header,0,&[])),1,"save").err().expect("child error");assert_eq!(error.code,ErrorCode::StorageFailure);assert!(!terminal_error(&error));
    let mut stream=Cursor::new(framed(header,u32::MAX,&[]));let error=decode_response(&mut stream,1,"save").err().expect("invalid body");assert!(terminal_error(&error));assert_eq!(stream.position(),8+header.len()as u64);
    let mut incomplete=framed(header,0,&[]);incomplete.pop();assert!(terminal_error(&decode_response(&mut Cursor::new(incomplete),1,"save").err().expect("truncated length")));
}
#[test]
fn method_bounds_and_exact_binary_round_trip(){
    let header=br#"{"protocol":1,"id":1,"status":"ok","result":{}}"#;
    assert!(decode_response(&mut Cursor::new(framed(header,1,&[7])),1,"status").is_err());
    assert!(decode_response(&mut Cursor::new(framed(header,0,&[])),1,"save").is_err());
    assert!(decode_response(&mut Cursor::new(framed(header,8,&[7])),1,"save").is_err());
    let encoded=encode_request(&request("save",json!({"kind":"in_game"})),0).expect("encode");let mut sent=Vec::new();
    let result=exchange(&mut sent,&mut Cursor::new(framed(header,4,&[0,1,254,255])),1,"save",&encoded,&[]).expect("exchange");
    assert_eq!(result.payload,[0,1,254,255]);assert_eq!(sent,framed(&encoded,0,&[]));
    let mut oversized=Cursor::new((MAX_HEADER as u32+1).to_le_bytes().to_vec());assert!(decode_response(&mut oversized,1,"save").is_err());assert_eq!(oversized.position(),4);
}
#[test]
fn local_encoding_is_bounded_before_send(){
    assert_eq!(encode_request(&request("start",json!({})),1).err().expect("payload bound").code,ErrorCode::ResourceLimit);
    assert!(encode_request(&request("load",json!({})),MAX_SAVE_BYTES as usize).is_ok());
    assert!(encode_request(&request("load",json!({})),MAX_SAVE_BYTES as usize+1).is_err());
    assert!(encode_request(&request("unknown",json!({})),0).is_err());
    assert!(encode_request(&request("status",json!([])),0).is_err());
    assert!(encode_request(&request("status",json!({"blob":"x".repeat(MAX_HEADER)})),0).is_err());
    let mut nested=Value::Null;for _ in 0..31{nested=json!({"n":nested});}assert!(encode_request(&request("status",nested.clone()),0).is_ok());nested=json!({"n":nested});assert!(encode_request(&request("status",nested),0).is_err());
    let mut buffer=HeaderBuffer(Vec::new());buffer.write_all(&vec![0;MAX_HEADER]).expect("exact cap");assert!(buffer.write_all(&[1]).is_err());assert_eq!(buffer.0.len(),MAX_HEADER);
}
struct ProcessCounter{terminations:AtomicUsize}
impl WorkerProcess for ProcessCounter{fn has_exited(&self)->bool{false}fn terminate(&self){self.terminations.fetch_add(1,Ordering::SeqCst);}}
#[test]
fn cancellation_latches_channel_without_admitting_a_future_command(){
    let(sender,receiver)=mpsc::sync_channel(1);let mut channel=WorkerChannel{sender:Some(sender),thread:None,sequence:0};let process=ProcessCounter{terminations:AtomicUsize::new(0)};
    let error=channel.call_cancellable(&process,"status",json!({}),Vec::new(),Duration::from_secs(1),&AtomicBool::new(true)).err().expect("cancel");assert_eq!(error.code,ErrorCode::Cancelled);assert!(channel.sender.is_none());assert_eq!(channel.sequence,0);assert!(receiver.try_recv().is_err());
    assert!(channel.call(&process,"status",json!({}),Vec::new(),Duration::from_secs(1)).is_err());assert_eq!(process.terminations.load(Ordering::SeqCst),1);
}
#[test]
fn local_rejection_preserves_sequence_but_timeout_is_terminal(){
    let(sender,receiver)=mpsc::sync_channel(1);let mut channel=WorkerChannel{sender:Some(sender),thread:None,sequence:0};let process=ProcessCounter{terminations:AtomicUsize::new(0)};
    assert!(channel.call(&process,"status",json!({}),vec![1],Duration::from_secs(1)).is_err());assert_eq!(channel.sequence,0);assert!(channel.sender.is_some());assert!(receiver.try_recv().is_err());assert_eq!(process.terminations.load(Ordering::SeqCst),0);
    assert!(channel.call(&process,"status",json!({}),Vec::new(),Duration::from_millis(2)).is_err());assert!(channel.sender.is_none());assert_eq!(channel.sequence,1);assert_eq!(receiver.try_recv().expect("queued before timeout").id,1);assert_eq!(process.terminations.load(Ordering::SeqCst),1);
}
#[test]
fn exhausted_sequence_cannot_wrap_or_queue_another_call(){
    let(sender,receiver)=mpsc::sync_channel(1);let mut channel=WorkerChannel{sender:Some(sender),thread:None,sequence:MAX_SEQUENCE};let process=ProcessCounter{terminations:AtomicUsize::new(0)};
    assert!(channel.call(&process,"status",json!({}),Vec::new(),Duration::from_secs(1)).is_err());assert!(channel.sender.is_none());assert_eq!(channel.sequence,MAX_SEQUENCE);assert!(receiver.try_recv().is_err());assert_eq!(process.terminations.load(Ordering::SeqCst),1);
}
#[test]
fn queue_refusal_invalidates_without_consuming_the_identity(){
    let(sender,_receiver)=mpsc::sync_channel(0);let mut channel=WorkerChannel{sender:Some(sender),thread:None,sequence:0};let process=ProcessCounter{terminations:AtomicUsize::new(0)};
    assert!(channel.call(&process,"status",json!({}),Vec::new(),Duration::from_secs(1)).is_err());assert!(channel.sender.is_none());assert_eq!(channel.sequence,0);assert_eq!(process.terminations.load(Ordering::SeqCst),1);
}
