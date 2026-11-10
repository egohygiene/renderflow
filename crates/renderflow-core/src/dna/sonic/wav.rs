//! Byte inspection for Sonic's selected native music WAV snapshot. This never
//! converts, resamples, downmixes, clamps or invokes an audio decoder.
use anyhow::{ensure,Context,Result};
use crate::process::ProcessCancellationToken;
#[derive(Debug,Clone,Copy)]
pub(super) struct WavInspection {
    pub duration_ms:u64,pub frame_count:u64,pub sample_rate_hz:u32,pub channels:u16,
    pub bits_per_sample:u16,pub encoding:&'static str,
}
impl WavInspection {
    pub fn frame_at_millisecond(self,time:u64)->Result<u64> {
        let value=u128::from(time)*u128::from(self.sample_rate_hz);
        u64::try_from(value.div_ceil(1000)).context("sonic.wav.clock.overflow")
    }
}
pub(super) fn inspect(bytes:&[u8],token:&ProcessCancellationToken)->Result<WavInspection> {
    ensure!(!token.is_cancelled(),"sonic.cancelled");
    ensure!((44..=256*1024*1024).contains(&bytes.len()) && &bytes[..4]==b"RIFF" && &bytes[8..12]==b"WAVE","sonic.wav.riff");
    let length=u32::from_le_bytes(bytes[4..8].try_into()?) as u64;
    ensure!(length+8==bytes.len() as u64,"sonic.wav.riff_length");
    let mut offset=12usize;let mut chunks=0;let mut format=None;let mut data=None;let mut fact=None;
    while offset<bytes.len() {
        ensure!(!token.is_cancelled(),"sonic.cancelled");chunks+=1;
        ensure!(chunks<=256&&bytes.len()-offset>=8,"sonic.wav.chunks");
        let id=&bytes[offset..offset+4];let size=u32::from_le_bytes(bytes[offset+4..offset+8].try_into()?) as usize;
        let start=offset+8;let end=start.checked_add(size).context("sonic.wav.chunk.overflow")?;let next=end.checked_add(size&1).context("sonic.wav.chunk.overflow")?;
        ensure!(next<=bytes.len(),"sonic.wav.chunk.bounds");
        match id {
            b"fmt "=>{
                ensure!(format.is_none()&&data.is_none()&&matches!(size,16|18),"sonic.wav.format");
                let f=&bytes[start..end];let word=|i|u16::from_le_bytes([f[i],f[i+1]]);
                let tag=word(0);let channels=word(2);let rate=u32::from_le_bytes(f[4..8].try_into()?);let byte_rate=u32::from_le_bytes(f[8..12].try_into()?);let block=word(12);let bits=word(14);
                let encoding=match (tag,bits) {(1,16)=>"pcm_s16le",(1,24)=>"pcm_s24le",(1,32)=>"pcm_s32le",(3,32)=>"ieee_float32_le",_=>anyhow::bail!("sonic.wav.encoding: native PCM16/24/32 or finite IEEE float32 required; compressed/extensible/RF64 unsupported")};
                ensure!((1..=8).contains(&channels)&&(8_000..=192_000).contains(&rate)&&block==channels*(bits/8)&&byte_rate==rate*u32::from(block)&&(size==16||word(16)==0),"sonic.wav.format_clock");
                format=Some((rate,channels,bits,block,encoding));
            },
            b"data"=>{ensure!(format.is_some()&&data.is_none()&&size>0,"sonic.wav.data");data=Some((start,end));},
            b"fact"=>{ensure!(fact.is_none()&&size==4,"sonic.wav.fact");fact=Some(u32::from_le_bytes(bytes[start..end].try_into()?) as u64);},
            _=>{}, // Ancillary bytes remain in the exact source digest, not facts.
        }
        offset=next;
    }
    let (rate,channels,bits,block,encoding)=format.context("sonic.wav.missing_format")?;let (start,end)=data.context("sonic.wav.missing_data")?;
    let size=(end-start) as u64;ensure!(size%u64::from(block)==0,"sonic.wav.partial_frame");let frames=size/u64::from(block);
    ensure!(frames>0&&frames<=u64::from(rate)*600&&fact.is_none_or(|n|n==frames),"sonic.wav.frame_count_or_duration");
    if encoding=="ieee_float32_le" {
        for (index,word) in bytes[start..end].chunks_exact(4).enumerate() {
            if index%65_536==0 {ensure!(!token.is_cancelled(),"sonic.cancelled");}
            ensure!(f32::from_le_bytes(word.try_into()?).is_finite(),"sonic.wav.nonfinite_sample");
        }
    }
    Ok(WavInspection {duration_ms:(frames*1000).div_ceil(u64::from(rate)),frame_count:frames,sample_rate_hz:rate,channels,bits_per_sample:bits,encoding})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wave(tag:u16,bits:u16,channels:u16,rate:u32,payload:&[u8])->Vec<u8> {
        let block=channels*(bits/8);let mut b=b"RIFF".to_vec();b.extend((36+payload.len() as u32+(payload.len() as u32&1)).to_le_bytes());b.extend(b"WAVEfmt ");b.extend(16u32.to_le_bytes());b.extend(tag.to_le_bytes());b.extend(channels.to_le_bytes());b.extend(rate.to_le_bytes());b.extend((rate*u32::from(block)).to_le_bytes());b.extend(block.to_le_bytes());b.extend(bits.to_le_bytes());b.extend(b"data");b.extend((payload.len() as u32).to_le_bytes());b.extend(payload);if payload.len()%2!=0 {b.push(0);}b
    }
    #[test]
    fn native_stereo_24_bit_frames_and_fractional_millisecond_clock_are_preserved() {
        let bytes=wave(1,24,2,44_100,&vec![0;44_100*6]);let before=bytes.clone();let value=inspect(&bytes,&ProcessCancellationToken::new()).unwrap();
        assert_eq!(value.frame_count,44_100);assert_eq!(value.channels,2);assert_eq!(value.bits_per_sample,24);assert_eq!(value.duration_ms,1000);assert_eq!(value.frame_at_millisecond(1).unwrap(),45);assert_eq!(bytes,before);
        assert!(crate::speech::provider::inspect_pcm_wav_details(&bytes).is_err());
    }
    #[test]
    fn float_headroom_is_preserved_while_nonfinite_payload_is_refused() {
        for value in [2.0f32,-2.0,0.0] {let bytes=wave(3,32,1,48_000,&value.to_le_bytes());let read=inspect(&bytes,&ProcessCancellationToken::new()).unwrap();assert_eq!(read.encoding,"ieee_float32_le");assert_eq!(read.frame_count,1);}
        for value in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY] {assert!(inspect(&wave(3,32,1,48_000,&value.to_le_bytes()),&ProcessCancellationToken::new()).is_err());}
    }
    #[test]
    fn inconsistent_rate_alignment_trailing_bytes_and_extensible_encoding_refuse() {
        let original=wave(1,16,2,48_000,&[0;8]);for mutation in 0..4 {let mut bytes=original.clone();match mutation {0=>bytes[28]^=1,1=>bytes[32]=2,2=>bytes.push(0),_=>{bytes[20]=0xfe;bytes[21]=0xff;}}assert!(inspect(&bytes,&ProcessCancellationToken::new()).is_err());}
        assert!(inspect(&wave(1,24,2,44_100,&[0;5]),&ProcessCancellationToken::new()).is_err());
    }
}
