use anyhow::Result;

const RUN_VALUE_NAME: &str = "22Pie Graphic Service";

pub fn reconcile(enabled: bool) -> Result<()> {
    platform::reconcile(enabled)
}

fn startup_command(executable: &std::path::Path) -> String {
    format!("\"{}\"", executable.display())
}

#[cfg(windows)]
mod platform {
    use super::{startup_command, RUN_VALUE_NAME};
    use anyhow::{bail, Context, Result};
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            REG_SZ,
        },
    };

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(Some(0))
            .collect()
    }

    pub fn reconcile(enabled: bool) -> Result<()> {
        let subkey = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, subkey.as_ptr(), &mut key) };
        if status != ERROR_SUCCESS {
            bail!("could not open the current-user startup registry key ({status})");
        }
        let name = wide(RUN_VALUE_NAME);
        let result = if enabled {
            let executable =
                std::env::current_exe().context("could not locate GraphicService.exe")?;
            let command = wide(&startup_command(&executable));
            unsafe {
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr().cast(),
                    (command.len() * 2) as u32,
                )
            }
        } else {
            unsafe { RegDeleteValueW(key, name.as_ptr()) }
        };
        unsafe { RegCloseKey(key) };
        if result != ERROR_SUCCESS && !(result == ERROR_FILE_NOT_FOUND && !enabled) {
            bail!("could not update current-user startup registration ({result})");
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn reconcile(_enabled: bool) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_command_is_quoted_and_stable() {
        let path = std::path::Path::new(r"C:\Program Files\22Pie\GraphicService.exe");
        assert_eq!(
            startup_command(path),
            r#""C:\Program Files\22Pie\GraphicService.exe""#
        );
        assert_eq!(RUN_VALUE_NAME, "22Pie Graphic Service");
    }
}
