//! Optional console for the Windows GUI executable. Called before logging starts.

use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Console::{
    AllocConsole, AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS,
    STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

/// Reuse the launching shell, or create a console only when explicitly requested.
/// Preserve inherited pipes/files used by CI, launch probes and shell redirection.
pub fn init(create: bool) -> std::io::Result<()> {
    // SAFETY: These process-wide APIs run on the main thread before workers start.
    // Handles belong to the process; none are closed or transferred here.
    unsafe {
        let handles = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
            .map(|kind| (kind, GetStdHandle(kind)));
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            let error = std::io::Error::last_os_error();
            // Already attached (including the second call for --console).
            if error.raw_os_error() == Some(ERROR_ACCESS_DENIED.cast_signed()) {
                return Ok(());
            }
            if !create {
                return Ok(());
            }
            if AllocConsole() == 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        // AllocConsole/AttachConsole may replace the standard handle table.
        for (kind, handle) in handles {
            if !handle.is_null()
                && handle != INVALID_HANDLE_VALUE
                && SetStdHandle(kind, handle) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    Ok(())
}
