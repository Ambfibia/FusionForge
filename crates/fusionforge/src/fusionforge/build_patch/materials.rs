use super::*;

#[cfg(windows)]
pub(super) fn render_glyphs(
    path: &Path,
    face_name: &str,
    pixel_height: i32,
    codes: &[u32],
    aliases: &BTreeMap<u32, u32>,
) -> Result<Vec<GlyphBitmap>, String> {
    use std::{ffi::c_void, mem::size_of, ptr};
    use windows_sys::Win32::Graphics::Gdi::{
        AddFontResourceExW, CreateCompatibleDC, CreateFontW, DeleteDC, DeleteObject,
        GetGlyphOutlineW, RemoveFontResourceExW, SelectObject, ANTIALIASED_QUALITY,
        CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, FIXED, FR_PRIVATE, FW_NORMAL,
        GDI_ERROR, GGO_GRAY8_BITMAP, GLYPHMETRICS, MAT2, OUT_TT_ONLY_PRECIS,
    };

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let path_w = wide(&path.to_string_lossy());
    let face_w = wide(face_name);
    unsafe {
        if AddFontResourceExW(path_w.as_ptr(), FR_PRIVATE, ptr::null_mut()) <= 0 {
            return Err(format!(
                "Windows could not load TTF font: {}",
                path.display()
            ));
        }
        let hdc = CreateCompatibleDC(ptr::null_mut());
        if hdc.is_null() {
            RemoveFontResourceExW(path_w.as_ptr(), FR_PRIVATE, ptr::null_mut());
            return Err("Windows could not create a GDI device context".to_string());
        }
        let font = CreateFontW(
            -pixel_height,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_TT_ONLY_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            ANTIALIASED_QUALITY as u32,
            DEFAULT_PITCH as u32,
            face_w.as_ptr(),
        );
        if font.is_null() {
            DeleteDC(hdc);
            RemoveFontResourceExW(path_w.as_ptr(), FR_PRIVATE, ptr::null_mut());
            return Err(format!("Windows could not create font face {face_name:?}"));
        }
        let old = SelectObject(hdc, font);
        let mat = MAT2 {
            eM11: FIXED { fract: 0, value: 1 },
            eM12: FIXED { fract: 0, value: 0 },
            eM21: FIXED { fract: 0, value: 0 },
            eM22: FIXED { fract: 0, value: 1 },
        };
        let mut result = Vec::new();
        for code in codes {
            let render_code = aliases.get(code).copied().unwrap_or(*code);
            let mut metrics: GLYPHMETRICS = std::mem::zeroed();
            let size = GetGlyphOutlineW(
                hdc,
                render_code,
                GGO_GRAY8_BITMAP,
                &mut metrics,
                0,
                ptr::null_mut(),
                &mat,
            );
            if size == GDI_ERROR as u32 {
                continue;
            }
            let width = metrics.gmBlackBoxX as usize;
            let height = metrics.gmBlackBoxY as usize;
            let advance = i32::from(metrics.gmCellIncX.max(0));
            if width == 0 || height == 0 {
                result.push(GlyphBitmap {
                    code: *code,
                    render_code,
                    width: 0,
                    height: 0,
                    origin_x: 0,
                    origin_y: 0,
                    advance,
                    rows: Vec::new(),
                });
                continue;
            }
            let mut buffer = vec![0u8; size as usize + size_of::<usize>()];
            let rendered = GetGlyphOutlineW(
                hdc,
                render_code,
                GGO_GRAY8_BITMAP,
                &mut metrics,
                size,
                buffer.as_mut_ptr() as *mut c_void,
                &mat,
            );
            if rendered == GDI_ERROR as u32 {
                continue;
            }
            let stride = (width + 3) & !3;
            let available = rendered as usize;
            let mut rows = Vec::new();
            for y in 0..height {
                let mut row = Vec::with_capacity(width);
                for x in 0..width {
                    let index = y * stride + x;
                    row.push(if index < available {
                        buffer[index].saturating_mul(4)
                    } else {
                        0
                    });
                }
                rows.push(row);
            }
            result.push(GlyphBitmap {
                code: *code,
                render_code,
                width,
                height,
                origin_x: metrics.gmptGlyphOrigin.x,
                origin_y: metrics.gmptGlyphOrigin.y,
                advance,
                rows,
            });
        }
        if !old.is_null() {
            SelectObject(hdc, old);
        }
        DeleteObject(font);
        DeleteDC(hdc);
        RemoveFontResourceExW(path_w.as_ptr(), FR_PRIVATE, ptr::null_mut());
        Ok(result)
    }
}

#[cfg(not(windows))]
pub(super) fn render_glyphs(
    _path: &Path,
    _face_name: &str,
    _pixel_height: i32,
    _codes: &[u32],
    _aliases: &BTreeMap<u32, u32>,
) -> Result<Vec<GlyphBitmap>, String> {
    Err("TTF bitmap font patching currently requires Windows GDI".to_string())
}
