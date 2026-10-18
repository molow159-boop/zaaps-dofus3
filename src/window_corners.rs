//! Clips the undecorated Windows window to subtly rounded corners.
//! The maximized window retains a rectangular region.

use std::ffi::c_void;

type WindowHandle = *mut c_void;
type RegionHandle = *mut c_void;

#[repr(C)]
#[derive(Default)]
struct WindowRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(WindowHandle, isize) -> i32>,
        data: isize,
    ) -> i32;
    fn GetWindowThreadProcessId(window: WindowHandle, process_id: *mut u32) -> u32;
    fn GetWindowTextW(window: WindowHandle, buffer: *mut u16, max_count: i32) -> i32;
    fn IsWindow(window: WindowHandle) -> i32;
    fn IsIconic(window: WindowHandle) -> i32;
    fn IsZoomed(window: WindowHandle) -> i32;
    fn GetWindowRect(window: WindowHandle, rect: *mut WindowRect) -> i32;
    fn SetWindowRgn(window: WindowHandle, region: RegionHandle, redraw: i32) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateRoundRectRgn(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        ellipse_width: i32,
        ellipse_height: i32,
    ) -> RegionHandle;
    fn DeleteObject(object: *mut c_void) -> i32;
}

#[derive(Default)]
struct WindowSearch {
    process_id: u32,
    found: isize,
}

unsafe extern "system" fn select_window(window: WindowHandle, context: isize) -> i32 {
    let search = &mut *(context as *mut WindowSearch);
    let mut process_id = 0u32;
    GetWindowThreadProcessId(window, &mut process_id);
    if process_id != search.process_id {
        return 1;
    }

    let mut title = [0u16; 80];
    let length = GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32);
    if length > 0 && String::from_utf16_lossy(&title[..length as usize]) == "Zaaps Dofus 3" {
        search.found = window as isize;
        return 0;
    }
    1
}

fn find_window() -> isize {
    let mut search = WindowSearch {
        process_id: std::process::id(),
        found: 0,
    };
    unsafe {
        EnumWindows(
            Some(select_window),
            (&mut search as *mut WindowSearch) as isize,
        );
    }
    search.found
}

fn region_dimensions(width: i32, height: i32, pixels_per_point: f32) -> Option<(i32, i32, i32)> {
    if width < 200 || height < 200 {
        return None;
    }
    let radius = (12.0 * pixels_per_point).round().clamp(10.0, 30.0) as i32;
    // The Win32 region uses an ellipse diameter, not a corner radius.
    Some((width, height, radius * 2))
}

#[derive(Default)]
pub struct WindowCorners {
    window: isize,
    last: Option<(i32, i32, i32, bool)>,
}

impl WindowCorners {
    pub fn update(&mut self, pixels_per_point: f32) {
        if self.window == 0 || unsafe { IsWindow(self.window as WindowHandle) } == 0 {
            self.window = find_window();
            self.last = None;
        }
        if self.window == 0 {
            return;
        }

        let window = self.window as WindowHandle;
        unsafe {
            if IsIconic(window) != 0 {
                return;
            }
            if IsZoomed(window) != 0 {
                if self.last.map(|state| state.3) != Some(true)
                    && SetWindowRgn(window, std::ptr::null_mut(), 1) != 0
                {
                    self.last = Some((0, 0, 0, true));
                }
                return;
            }

            let mut bounds = WindowRect::default();
            if GetWindowRect(window, &mut bounds) == 0 {
                return;
            }
            let Some((width, height, diameter)) = region_dimensions(
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                pixels_per_point,
            ) else {
                return;
            };
            let state = (width, height, diameter, false);
            if self.last == Some(state) {
                return;
            }

            let region = CreateRoundRectRgn(0, 0, width, height, diameter, diameter);
            if region.is_null() {
                return;
            }
            if SetWindowRgn(window, region, 1) != 0 {
                // Windows now owns the region; never free it here.
                self.last = Some(state);
            } else {
                DeleteObject(region);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::region_dimensions;

    #[test]
    fn rounded_radius_tracks_screen_scaling() {
        assert_eq!(region_dimensions(960, 750, 1.0), Some((960, 750, 24)));
        assert_eq!(region_dimensions(1440, 1125, 1.5), Some((1440, 1125, 36)));
    }

    #[test]
    fn too_small_or_invalid_window_is_not_clipped() {
        assert_eq!(region_dimensions(0, 750, 1.0), None);
        assert_eq!(region_dimensions(960, 0, 1.0), None);
    }
}
