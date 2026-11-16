//! Background tools must not create console windows when Nida has no console.

use std::ffi::OsStr;
use std::process::Command;

/// Build a helper command without changing its arguments, pipes or environment.
pub(crate) fn background_command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(target_os = "windows")]
    let command = {
        use std::os::windows::process::CommandExt as _;
        let mut command = command;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        command
    };
    command
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::background_command;

    #[test]
    fn background_tools_have_no_console_and_keep_their_pipes() {
        let output = background_command(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "internal::process::tests::console_probe",
                "--nocapture",
            ])
            .env("NIDA_TEST_BACKGROUND_CONSOLE", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("probe stdout"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("probe stderr"));
    }

    #[test]
    fn console_probe() {
        if std::env::var_os("NIDA_TEST_BACKGROUND_CONSOLE").is_none() {
            return;
        }
        // Windows may provide a hidden console/pseudoconsole. Only a visible
        // helper window is a failure, including when the parent uses --console.
        // SAFETY: The window comes from Windows; querying visibility owns nothing.
        let visible = unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(
                windows_sys::Win32::System::Console::GetConsoleWindow(),
            )
        };
        assert_eq!(visible, 0, "background helper opened a console window");
        println!("probe stdout");
        eprintln!("probe stderr");
    }
}
