use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use iceberg_core::config::BackgroundKind;
use windows::core::{IUnknown, HSTRING};
use windows::Win32::Foundation::{GENERIC_READ, RPC_E_CHANGED_MODE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory, IWICPalette,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

const PROXY_MAX: u32 = 960;

pub fn needs_wallpaper(kind: BackgroundKind) -> bool {
    matches!(
        kind,
        BackgroundKind::Acrylic | BackgroundKind::Ice | BackgroundKind::Liquid
    )
}

#[derive(Clone)]
pub struct BackdropCrop {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

pub struct Wallpaper {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Wallpaper {
    pub fn load_from(path: &Path) -> Option<Self> {
        let _com = ComGuard::init()?;
        // SAFETY: this thread owns the COM initialization guard and the factory is used
        // synchronously before the guard is dropped.
        let factory: IWICImagingFactory = unsafe {
            CoCreateInstance(
                &CLSID_WICImagingFactory,
                None::<&IUnknown>,
                CLSCTX_INPROC_SERVER,
            )
        }
        .ok()?;
        let name = HSTRING::from(path.to_string_lossy().as_ref());
        // SAFETY: the path is a valid UTF-16 string and the decoder is released before
        // the COM guard.
        let decoder = unsafe {
            factory.CreateDecoderFromFilename(
                &name,
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnLoad,
            )
        }
        .ok()?;
        // SAFETY: WIC decoders expose a first frame for the image formats Windows uses for
        // wallpapers. A missing frame is treated as an unavailable wallpaper.
        let frame = unsafe { decoder.GetFrame(0) }.ok()?;
        let mut width = 0u32;
        let mut height = 0u32;
        // SAFETY: both out-pointers are valid stack slots.
        unsafe { frame.GetSize(&mut width, &mut height) }.ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let (target_w, target_h) = proxy_size(width, height, PROXY_MAX);
        // SAFETY: the scaler is initialized from the decoded frame and writes only its
        // own pixel buffer.
        let scaler = unsafe { factory.CreateBitmapScaler() }.ok()?;
        unsafe { scaler.Initialize(&frame, target_w, target_h, WICBitmapInterpolationModeFant) }
            .ok()?;
        // SAFETY: converting to BGRA matches the buffer below and is the format the CPU
        // backdrop pipeline consumes.
        let converter = unsafe { factory.CreateFormatConverter() }.ok()?;
        unsafe {
            converter.Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
        }
        .ok()?;
        let mut bgra = vec![0u8; target_w as usize * target_h as usize * 4];
        // SAFETY: the buffer length and stride describe the same target dimensions.
        unsafe { converter.CopyPixels(std::ptr::null(), target_w * 4, &mut bgra) }.ok()?;
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        Self::from_rgba(target_w as usize, target_h as usize, bgra)
    }

    pub fn from_rgba(width: usize, height: usize, rgba: Vec<u8>) -> Option<Self> {
        if width == 0 || height == 0 || rgba.len() != width * height * 4 {
            return None;
        }
        Some(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn crop_cover(
        &self,
        monitor: [i32; 4],
        window: [i32; 4],
        out_w: usize,
        out_h: usize,
    ) -> BackdropCrop {
        let out_w = out_w.max(1);
        let out_h = out_h.max(1);
        let monitor_w = monitor[2].max(1) as f64;
        let monitor_h = monitor[3].max(1) as f64;
        let scale = (monitor_w / self.width as f64).max(monitor_h / self.height as f64);
        let displayed_w = self.width as f64 * scale;
        let displayed_h = self.height as f64 * scale;
        let display_x = monitor[0] as f64 + (monitor_w - displayed_w) / 2.0;
        let display_y = monitor[1] as f64 + (monitor_h - displayed_h) / 2.0;
        let window_w = window[2].max(1) as f64;
        let window_h = window[3].max(1) as f64;
        let step_x = window_w / out_w as f64;
        let step_y = window_h / out_h as f64;
        let mut rgba = vec![0u8; out_w * out_h * 4];
        for oy in 0..out_h {
            for ox in 0..out_w {
                let source_x =
                    (window[0] as f64 + (ox as f64 + 0.5) * step_x - display_x) / scale - 0.5;
                let source_y =
                    (window[1] as f64 + (oy as f64 + 0.5) * step_y - display_y) / scale - 0.5;
                let pixel =
                    sample_bilinear(&self.rgba, self.width, self.height, source_x, source_y);
                let index = (oy * out_w + ox) * 4;
                rgba[index..index + 4].copy_from_slice(&pixel);
            }
        }
        BackdropCrop {
            width: out_w,
            height: out_h,
            rgba,
        }
    }
}

pub fn wallpaper_path() -> Option<PathBuf> {
    let mut buffer = vec![0u16; 32_768];
    // SAFETY: the buffer is writable for the advertised character count; the API treats
    // the path as a NUL-terminated UTF-16 string.
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    result.ok()?;
    let length = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    if length == 0 {
        return None;
    }
    let path = PathBuf::from(OsString::from_wide(&buffer[..length]));
    path.is_file().then_some(path)
}

struct ComGuard {
    uninitialize: bool,
}

impl ComGuard {
    fn init() -> Option<Self> {
        // SAFETY: the matching CoUninitialize is in Drop on every successful path.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Some(Self {
                uninitialize: false,
            });
        }
        if result.is_ok() {
            return Some(Self { uninitialize: true });
        }
        None
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            // SAFETY: balances the successful CoInitializeEx above.
            unsafe { CoUninitialize() };
        }
    }
}

fn proxy_size(width: u32, height: u32, max: u32) -> (u32, u32) {
    let largest = width.max(height);
    if largest <= max {
        return (width.max(1), height.max(1));
    }
    let scale = max as f64 / largest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

fn sample_bilinear(image: &[u8], width: usize, height: usize, x: f64, y: f64) -> [u8; 4] {
    let x = x.clamp(0.0, width.saturating_sub(1) as f64);
    let y = y.clamp(0.0, height.saturating_sub(1) as f64);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width.saturating_sub(1));
    let y1 = (y0 + 1).min(height.saturating_sub(1));
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let mut pixel = [0u8; 4];
    for (channel, value) in pixel.iter_mut().enumerate() {
        let top = image[(y0 * width + x0) * 4 + channel] as f64 * (1.0 - tx)
            + image[(y0 * width + x1) * 4 + channel] as f64 * tx;
        let bottom = image[(y1 * width + x0) * 4 + channel] as f64 * (1.0 - tx)
            + image[(y1 * width + x1) * 4 + channel] as f64 * tx;
        *value = (top * (1.0 - ty) + bottom * ty).round().clamp(0.0, 255.0) as u8;
    }
    pixel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripe_wallpaper() -> Wallpaper {
        let mut rgba = vec![0u8; 2 * 2 * 4];
        for y in 0..2 {
            for x in 0..2 {
                let index = (y * 2 + x) * 4;
                let color = if x == 0 { [255, 0, 0] } else { [0, 0, 255] };
                rgba[index] = color[0];
                rgba[index + 1] = color[1];
                rgba[index + 2] = color[2];
                rgba[index + 3] = 255;
            }
        }
        Wallpaper::from_rgba(2, 2, rgba).expect("wallpaper")
    }

    #[test]
    fn invalid_rgba_is_rejected() {
        assert!(Wallpaper::from_rgba(0, 2, vec![0; 16]).is_none());
        assert!(Wallpaper::from_rgba(2, 2, vec![0; 12]).is_none());
    }

    #[test]
    fn crop_keeps_requested_size_and_samples_the_image() {
        let crop = stripe_wallpaper().crop_cover([0, 0, 2, 2], [0, 0, 2, 2], 2, 2);
        assert_eq!((crop.width, crop.height), (2, 2));
        assert_eq!(&crop.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&crop.rgba[4..8], &[0, 0, 255, 255]);
    }

    #[test]
    fn cover_crop_scales_a_letterboxed_monitor() {
        let crop = stripe_wallpaper().crop_cover([0, 0, 4, 2], [0, 0, 4, 2], 4, 2);
        assert_eq!((crop.width, crop.height), (4, 2));
        assert!(crop.rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn proxy_size_preserves_aspect_ratio() {
        assert_eq!(proxy_size(1920, 1080, 960), (960, 540));
        assert_eq!(proxy_size(800, 600, 960), (800, 600));
    }
}
