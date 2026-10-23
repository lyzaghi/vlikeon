//! Owned SDR reference conversion. Not a capture API, scaler, HDR tonemapper or
//! GPU path. Actual receiver colour/VUI validation remains required.
use crate::{invalid,native_video::{Binding,ColourEncoding,Frame,PixelFormat,Profile}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::sync::OnceLock;

static SRGB_TO_709:OnceLock<[f64;256]>=OnceLock::new();
fn transfer()->&'static[f64;256]{
    SRGB_TO_709.get_or_init(||std::array::from_fn(|index|{
        let srgb=index as f64/255.0;
        let linear=if srgb<=0.04045{srgb/12.92}else{((srgb+0.055)/1.055).powf(2.4)};
        if linear<0.018{4.5*linear}else{1.099*linear.powf(0.45)-0.099}
    }))
}
fn allocation(length:usize)->Result<Vec<u8>>{
    let mut bytes=Vec::new();bytes.try_reserve_exact(length).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The native video conversion buffer is unavailable"))?;
    bytes.resize(length,0);Ok(bytes)
}
fn quantize(value:f64,minimum:f64,maximum:f64)->u8{value.round().clamp(minimum,maximum)as u8}

/// Preserve identity/sequence/timestamps; normalize padded NV12 or convert opaque
/// full-range sRGB BGRA into tight limited-range BT.709 NV12. Chroma is the 2x2
/// average of unquantized Cb/Cr, centered horizontally and vertically. Recheck
/// native authority/age AFTER this CPU work, before encoder submission.
pub fn tight_nv12(frame:Frame,binding:Binding,profile:Profile)->Result<Frame>{
    profile.validate()?;frame.validate_for_conversion(binding,profile)?;
    let width=frame.width as usize;let height=frame.height as usize;let stride=frame.stride as usize;
    let luma=width.checked_mul(height).ok_or_else(invalid)?;
    if frame.format==PixelFormat::Nv12&&stride==width{return Ok(frame);}
    let mut bytes=allocation(luma+luma/2)?;
    match frame.format{
        PixelFormat::Nv12=>{
            for row in 0..height+height/2{bytes[row*width..(row+1)*width].copy_from_slice(&frame.bytes[row*stride..row*stride+width]);}
        }
        PixelFormat::Bgra8=>{
            let lut=transfer();
            for row in (0..height).step_by(2){
                for column in (0..width).step_by(2){
                    let(mut cb,mut cr)=(0.0,0.0);
                    for dy in 0..2{
                        for dx in 0..2{
                            let offset=(row+dy)*stride+(column+dx)*4;
                            if frame.bytes[offset+3]!=255{return Err(invalid());}
                            let b=lut[frame.bytes[offset]as usize];let g=lut[frame.bytes[offset+1]as usize];let r=lut[frame.bytes[offset+2]as usize];
                            let y=0.2126*r+0.7152*g+0.0722*b;
                            bytes[(row+dy)*width+column+dx]=quantize(16.0+219.0*y,16.0,235.0);
                            cb+=(b-y)/1.8556;cr+=(r-y)/1.5748;
                        }
                    }
                    let uv=luma+(row/2)*width+column;
                    bytes[uv]=quantize(128.0+224.0*cb/4.0,16.0,240.0);
                    bytes[uv+1]=quantize(128.0+224.0*cr/4.0,16.0,240.0);
                }
            }
        }
    }
    Ok(Frame{binding:frame.binding,sequence:frame.sequence,timestamp_us:frame.timestamp_us,
        width:frame.width,height:frame.height,format:PixelFormat::Nv12,colour:ColourEncoding::Bt709LimitedCentered,
        stride:frame.width,bytes})
}

#[cfg(test)]
mod tests{
    use super::*;
    use uuid::Uuid;
    fn profile()->Profile{Profile{width:1280,height:720,fps:60,bitrate_bps:8_000_000}}
    fn binding()->Binding{Binding{capture:Uuid::from_u128(1),session:Uuid::from_u128(2),generation:3}}
    fn solid(bgra:[u8;4])->Frame{
        let p=profile();let stride=p.width*4+16;let mut bytes=vec![0xee;(stride*p.height)as usize];
        for row in 0..p.height as usize{for column in 0..p.width as usize{
            let offset=row*stride as usize+column*4;bytes[offset..offset+4].copy_from_slice(&bgra);
        }}
        Frame{binding:binding(),sequence:7,timestamp_us:12345,width:p.width,height:p.height,format:PixelFormat::Bgra8,
            colour:ColourEncoding::SrgbOpaque,stride,bytes}
    }
    #[test]
    fn solid_primary_reference_vectors(){
        // Independent BT.709 endpoint vectors: primaries survive either transfer
        // curve. Expected rounded studio-range codes come from the standard's
        // matrix/ranges, not from the converter's transfer helper.
        for(bgra,y,uv)in [([0,0,0,255],16,[128,128]),([255,255,255,255],235,[128,128]),
            ([0,0,255,255],63,[102,240]),([0,255,0,255],173,[42,26]),([255,0,0,255],32,[240,118])]{
            let frame=tight_nv12(solid(bgra),binding(),profile()).unwrap();let luma=(frame.width*frame.height)as usize;
            assert!(frame.bytes[..luma].iter().all(|value|*value==y));
            assert!(frame.bytes[luma..].chunks_exact(2).all(|value|value==uv));
        }
    }
    #[test]
    fn preserves_binding_time_sequence_and_removes_padding(){
        let frame=tight_nv12(solid([0,0,0,255]),binding(),profile()).unwrap();
        assert_eq!(frame.binding,binding());assert_eq!(frame.sequence,7);assert_eq!(frame.timestamp_us,12345);
        assert_eq!(frame.stride,1280);assert_eq!(frame.bytes.len(),1280*720*3/2);
        assert_eq!(frame.colour,ColourEncoding::Bt709LimitedCentered);
    }
    #[test]
    fn rejects_uncomposited_alpha(){let mut frame=solid([0,0,0,255]);frame.bytes[3]=128;assert!(tight_nv12(frame,binding(),profile()).is_err());}
    #[test]
    fn rejects_mismatched_colour_contract(){let mut frame=solid([0,0,0,255]);frame.colour=ColourEncoding::Bt709LimitedCentered;assert!(tight_nv12(frame,binding(),profile()).is_err());}
    #[test]
    fn rejects_foreign_generation(){let mut foreign=binding();foreign.generation+=1;assert!(tight_nv12(solid([0,0,0,255]),foreign,profile()).is_err());}
    #[test]
    fn normalizes_padded_nv12_planes(){
        let p=profile();let stride=p.width+16;let mut bytes=vec![0xee;(stride*(p.height+p.height/2))as usize];
        for row in 0..(p.height+p.height/2)as usize{
            bytes[row*stride as usize..row*stride as usize+p.width as usize].fill(if row<p.height as usize{16}else{128});
        }
        let frame=Frame{binding:binding(),sequence:1,timestamp_us:0,width:p.width,height:p.height,format:PixelFormat::Nv12,
            colour:ColourEncoding::Bt709LimitedCentered,stride,bytes};
        let result=tight_nv12(frame,binding(),p).unwrap();let luma=(p.width*p.height)as usize;
        assert!(result.bytes[..luma].iter().all(|value|*value==16));assert!(result.bytes[luma..].iter().all(|value|*value==128));
    }
    #[test]
    fn rejects_incomplete_buffer(){let mut frame=solid([0,0,0,255]);frame.bytes.pop();assert!(tight_nv12(frame,binding(),profile()).is_err());}
    #[test]
    fn rejects_changed_dimensions(){let mut frame=solid([0,0,0,255]);frame.height+=2;assert!(tight_nv12(frame,binding(),profile()).is_err());}
}
