use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
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
            constrain_maximized_width(bounds);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::MINMAXINFO;

    use super::constrain_maximized_width;

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
}
