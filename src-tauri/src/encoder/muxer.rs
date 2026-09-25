use crate::audio::mixer;
use crate::logging;
use crate::models::Quality;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSinkWriter, IMFMediaType, MFCreateAttributes,
    MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL,
    MFSTARTUP_FULL, MFShutdown, MFStartup, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_MT_FRAME_SIZE,
    MF_MT_FRAME_RATE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_AVG_BITRATE, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive, MFMediaType_Video, MFMediaType_Audio,
    MFVideoFormat_H264, MFVideoFormat_ARGB32, MFAudioFormat_AAC, MFAudioFormat_Float,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_VERSION,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

pub struct Mp4Writer {
    sink: IMFSinkWriter,
    video_stream_index: u32,
    audio_stream_index: Option<u32>,
    width: u32,
    height: u32,
    fps: u32,
    has_audio: bool,
}

impl Mp4Writer {
    pub fn create(
        path: &Path,
        width: u32,
        height: u32,
        fps: u32,
        quality: Quality,
        include_audio: bool,
    ) -> Result<Self, String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| format!("MFStartup: {e}"))?;
        }

        let mut attributes: Option<IMFAttributes> = None;
        unsafe {
            MFCreateAttributes(&mut attributes, 1).map_err(|e| e.to_string())?;
        }
        let attributes = attributes.ok_or("MF attributes missing")?;
        unsafe {
            attributes
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(|e| e.to_string())?;
        }

        let path_wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let sink = unsafe {
            MFCreateSinkWriterFromURL(PCWSTR(path_wide.as_ptr()), None, Some(&attributes))
                .map_err(|e| format!("MFCreateSinkWriterFromURL: {e}"))?
        };

        let video_stream_index = add_video_stream(&sink, width, height, fps, quality)?;
        let audio_stream_index = if include_audio {
            Some(add_audio_stream(&sink, quality)?)
        } else {
            None
        };

        unsafe {
            sink.BeginWriting().map_err(|e| e.to_string())?;
        }

        logging::info(format!(
            "MP4 writer created: {} ({}x{} @ {}fps, audio={})",
            path.display(),
            width,
            height,
            fps,
            include_audio
        ));

        Ok(Self {
            sink,
            video_stream_index,
            audio_stream_index,
            width,
            height,
            fps,
            has_audio: include_audio,
        })
    }

    pub fn write_video_frame(&mut self, bgra: &[u8], timestamp_100ns: i64) -> Result<(), String> {
        let expected = (self.width * self.height * 4) as usize;
        if bgra.len() < expected {
            return Err("Video frame buffer too small".into());
        }

        let buffer = unsafe {
            MFCreateMemoryBuffer(expected as u32).map_err(|e| e.to_string())?
        };

        unsafe {
            let mut data = std::ptr::null_mut();
            let mut max_len = 0u32;
            let mut cur_len = 0u32;
            buffer
                .Lock(&mut data, Some(&mut max_len), Some(&mut cur_len))
                .map_err(|e| e.to_string())?;
            std::ptr::copy_nonoverlapping(bgra.as_ptr(), data, expected);
            buffer.Unlock().map_err(|e| e.to_string())?;
            buffer
                .SetCurrentLength(expected as u32)
                .map_err(|e| e.to_string())?;
        }

        let sample = unsafe { MFCreateSample().map_err(|e| e.to_string())? };
        unsafe {
            sample.AddBuffer(&buffer).map_err(|e| e.to_string())?;
            sample
                .SetSampleTime(timestamp_100ns)
                .map_err(|e| e.to_string())?;
            let duration = 10_000_000i64 / self.fps as i64;
            sample
                .SetSampleDuration(duration)
                .map_err(|e| e.to_string())?;
            self.sink
                .WriteSample(self.video_stream_index, &sample)
                .map_err(|e| format!("WriteSample video: {e}"))?;
        }
        Ok(())
    }

    pub fn write_audio_pcm(&mut self, samples: &[f32], timestamp_100ns: i64) -> Result<(), String> {
        if !self.has_audio {
            return Ok(());
        }
        let stream = self.audio_stream_index.ok_or("No audio stream")?;
        let byte_len = samples.len() * 4;
        let buffer = unsafe {
            MFCreateMemoryBuffer(byte_len as u32).map_err(|e| e.to_string())?
        };

        unsafe {
            let mut data = std::ptr::null_mut();
            let mut max_len = 0u32;
            let mut cur_len = 0u32;
            buffer
                .Lock(&mut data, Some(&mut max_len), Some(&mut cur_len))
                .map_err(|e| e.to_string())?;
            let dst = std::slice::from_raw_parts_mut(data as *mut f32, samples.len());
            dst.copy_from_slice(samples);
            buffer.Unlock().map_err(|e| e.to_string())?;
            buffer
                .SetCurrentLength(byte_len as u32)
                .map_err(|e| e.to_string())?;
        }

        let sample = unsafe { MFCreateSample().map_err(|e| e.to_string())? };
        unsafe {
            sample.AddBuffer(&buffer).map_err(|e| e.to_string())?;
            sample
                .SetSampleTime(timestamp_100ns)
                .map_err(|e| e.to_string())?;
            let duration = (samples.len() as i64 * 10_000_000)
                / (mixer::sample_rate() as i64 * mixer::channels() as i64);
            sample
                .SetSampleDuration(duration.max(1))
                .map_err(|e| e.to_string())?;
            self.sink
                .WriteSample(stream, &sample)
                .map_err(|e| format!("WriteSample audio: {e}"))?;
        }
        Ok(())
    }

    pub fn finalize(self) -> Result<(), String> {
        unsafe {
            self.sink
                .Finalize()
                .map_err(|e| format!("SinkWriter finalize failed: {e}"))?;
            MFShutdown().ok();
        }
        logging::info("MP4 writer finalized successfully");
        Ok(())
    }
}

fn add_video_stream(
    sink: &IMFSinkWriter,
    width: u32,
    height: u32,
    fps: u32,
    quality: Quality,
) -> Result<u32, String> {
    let out_type = unsafe { MFCreateMediaType().map_err(|e| e.to_string())? };
    unsafe {
        out_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| e.to_string())?;
        out_type
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
            .map_err(|e| e.to_string())?;
    }
    set_frame_size(&out_type, width, height)?;
    set_ratio(&out_type, &MF_MT_FRAME_RATE, fps, 1)?;
    set_ratio(&out_type, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1)?;
    unsafe {
        out_type
            .SetUINT32(&MF_MT_AVG_BITRATE, quality.video_bitrate_bps(width, height))
            .map_err(|e| e.to_string())?;
        out_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(|e| e.to_string())?;
    }

    let stream_index = unsafe {
        sink.AddStream(&out_type).map_err(|e| e.to_string())?
    };

    let in_type = unsafe { MFCreateMediaType().map_err(|e| e.to_string())? };
    unsafe {
        in_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| e.to_string())?;
        in_type
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_ARGB32)
            .map_err(|e| e.to_string())?;
    }
    set_frame_size(&in_type, width, height)?;
    set_ratio(&in_type, &MF_MT_FRAME_RATE, fps, 1)?;
    set_ratio(&in_type, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1)?;

    unsafe {
        sink.SetInputMediaType(stream_index, &in_type, None)
            .map_err(|e| format!("SetInputMediaType video: {e}"))?;
    }

    Ok(stream_index)
}

fn add_audio_stream(sink: &IMFSinkWriter, quality: Quality) -> Result<u32, String> {
    let sample_rate = mixer::sample_rate();
    let channels = mixer::channels();
    let bitrate = quality.audio_bitrate_bps();

    let out_type = unsafe { MFCreateMediaType().map_err(|e| e.to_string())? };
    unsafe {
        out_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(|e| e.to_string())?;
        out_type
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)
            .map_err(|e| e.to_string())?;
        out_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels as u32)
            .map_err(|e| e.to_string())?;
        out_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)
            .map_err(|e| e.to_string())?;
        out_type
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bitrate / 8)
            .map_err(|e| e.to_string())?;
    }

    let stream_index = unsafe {
        sink.AddStream(&out_type).map_err(|e| e.to_string())?
    };

    let in_type = unsafe { MFCreateMediaType().map_err(|e| e.to_string())? };
    unsafe {
        in_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(|e| e.to_string())?;
        in_type
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)
            .map_err(|e| e.to_string())?;
        in_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels as u32)
            .map_err(|e| e.to_string())?;
        in_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)
            .map_err(|e| e.to_string())?;
        in_type
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 32)
            .map_err(|e| e.to_string())?;
        in_type
            .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, (channels * 4) as u32)
            .map_err(|e| e.to_string())?;
    }

    unsafe {
        sink.SetInputMediaType(stream_index, &in_type, None)
            .map_err(|e| format!("SetInputMediaType audio: {e}"))?;
    }

    Ok(stream_index)
}

fn set_frame_size(mt: &IMFMediaType, width: u32, height: u32) -> Result<(), String> {
    let packed = ((height as u64) << 32) | (width as u64);
    unsafe {
        mt.SetUINT64(&MF_MT_FRAME_SIZE, packed)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn set_ratio(mt: &IMFMediaType, key: &windows::core::GUID, num: u32, den: u32) -> Result<(), String> {
    let packed = ((den as u64) << 32) | (num as u64);
    unsafe {
        mt.SetUINT64(key, packed).map_err(|e| e.to_string())?;
    }
    Ok(())
}
