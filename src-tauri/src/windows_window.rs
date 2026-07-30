use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{MINMAXINFO, WM_GETMINMAXINFO};

const MAXIMIZE_WIDTH_SUBCLASS_ID: usize = 0x4350_4D57;

pub fn install_maximize_width_constraint(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let hwnd = window.hwnd()?;
    let installed = unsafe {
        SetWindowSubclass(
            hwnd,
            Some(maximize_width_subclass_proc),
            MAXIMIZE_WIDTH_SUBCLASS_ID,
            0,
        )
    };

    if installed.as_bool() {
        Ok(())
    } else {
        Err(tauri::Error::Anyhow(anyhow::anyhow!(
            "failed to install the Windows maximize width constraint: {}",
            windows::core::Error::from_win32()
        )))
    }
}

fn constrain_maximized_width(bounds: &mut MINMAXINFO) {
    let max_track_width = bounds.ptMaxTrackSize.x;
    if max_track_width > 0 && bounds.ptMaxSize.x > max_track_width {
        bounds.ptMaxSize.x = max_track_width;
    }
}

fn apply_monitor_work_area(bounds: &mut MINMAXINFO, monitor: RECT, work_area: RECT) -> bool {
    let work_width = work_area.right - work_area.left;
    let work_height = work_area.bottom - work_area.top;
    if work_width <= 0 || work_height <= 0 {
        return false;
    }

    bounds.ptMaxPosition.x = work_area.left - monitor.left;
    bounds.ptMaxPosition.y = work_area.top - monitor.top;
    bounds.ptMaxSize.x = work_width;
    bounds.ptMaxSize.y = work_height;
    constrain_maximized_width(bounds);
    true
}

fn constrain_maximized_bounds_to_work_area(hwnd: HWND, bounds: &mut MINMAXINFO) {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut monitor_info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    let has_monitor_info =
        !monitor.is_invalid() && unsafe { GetMonitorInfoW(monitor, &mut monitor_info).as_bool() };
    if !has_monitor_info
        || !apply_monitor_work_area(bounds, monitor_info.rcMonitor, monitor_info.rcWork)
    {
        constrain_maximized_width(bounds);
    }
}

unsafe extern "system" fn maximize_width_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _reference_data: usize,
) -> LRESULT {
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };

    if message == WM_GETMINMAXINFO {
        if let Some(bounds) = unsafe { (lparam.0 as *mut MINMAXINFO).as_mut() } {
            constrain_maximized_bounds_to_work_area(hwnd, bounds);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging::MINMAXINFO;

    use super::{apply_monitor_work_area, constrain_maximized_width};

    #[test]
    fn maximize_width_uses_the_existing_max_track_width() {
        let mut bounds = MINMAXINFO {
            ptMaxSize: POINT { x: 1920, y: 1080 },
            ptMaxTrackSize: POINT { x: 496, y: 1080 },
            ..Default::default()
        };

        constrain_maximized_width(&mut bounds);

        assert_eq!(bounds.ptMaxSize.x, 496);
        assert_eq!(bounds.ptMaxSize.y, 1080);
    }

    #[test]
    fn bottom_taskbar_limits_maximized_height_to_work_area() {
        let mut bounds = MINMAXINFO {
            ptMaxSize: POINT { x: 1920, y: 1080 },
            ptMaxTrackSize: POINT { x: 496, y: 1080 },
            ..Default::default()
        };

        apply_monitor_work_area(
            &mut bounds,
            RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
        );

        assert_eq!(bounds.ptMaxPosition, POINT { x: 0, y: 0 });
        assert_eq!(bounds.ptMaxSize, POINT { x: 496, y: 1040 });
    }

    #[test]
    fn top_taskbar_offsets_maximized_window() {
        let mut bounds = MINMAXINFO {
            ptMaxSize: POINT { x: 2560, y: 1440 },
            ptMaxTrackSize: POINT { x: 496, y: 1440 },
            ..Default::default()
        };

        apply_monitor_work_area(
            &mut bounds,
            RECT {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1440,
            },
            RECT {
                left: 0,
                top: 48,
                right: 2560,
                bottom: 1440,
            },
        );

        assert_eq!(bounds.ptMaxPosition, POINT { x: 0, y: 48 });
        assert_eq!(bounds.ptMaxSize, POINT { x: 496, y: 1392 });
    }

    #[test]
    fn side_taskbar_and_secondary_monitor_use_monitor_relative_position() {
        let mut bounds = MINMAXINFO {
            ptMaxSize: POINT { x: 1920, y: 1080 },
            ptMaxTrackSize: POINT { x: 496, y: 1080 },
            ..Default::default()
        };

        apply_monitor_work_area(
            &mut bounds,
            RECT {
                left: -1920,
                top: 120,
                right: 0,
                bottom: 1200,
            },
            RECT {
                left: -1872,
                top: 120,
                right: 0,
                bottom: 1200,
            },
        );

        assert_eq!(bounds.ptMaxPosition, POINT { x: 48, y: 0 });
        assert_eq!(bounds.ptMaxSize, POINT { x: 496, y: 1080 });
    }
}
