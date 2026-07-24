fn main() {
    match owstats_desktop_lib::parse_startup_mode(std::env::args_os()) {
        Ok(owstats_desktop_lib::StartupMode::Desktop) => {
            let exit = owstats_desktop_lib::handle_desktop_startup_result(
                owstats_desktop_lib::run(),
                show_desktop_startup_failure,
            );
            if exit != 0 {
                std::process::exit(exit);
            }
        }
        Ok(owstats_desktop_lib::StartupMode::PrepareInstallData {
            source,
            destination,
        }) => {
            let executable = match std::env::current_exe() {
                Ok(value) => value,
                Err(_) => std::process::exit(2),
            };
            if owstats_desktop_lib::run_install_data_mode(
                &executable,
                source.as_deref(),
                &destination,
            )
            .is_err()
            {
                std::process::exit(2);
            }
        }
        Ok(owstats_desktop_lib::StartupMode::DeleteInstallData { destination }) => {
            let executable = match std::env::current_exe() {
                Ok(value) => value,
                Err(_) => std::process::exit(2),
            };
            if owstats_desktop_lib::run_delete_install_data_mode(&executable, &destination).is_err()
            {
                std::process::exit(2);
            }
        }
        Err(_) => std::process::exit(2),
    }
}

#[cfg(windows)]
fn show_desktop_startup_failure(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let message = message
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let title = "Watchback 启动失败"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        let _ = MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_desktop_startup_failure(message: &str) {
    eprintln!("{message}");
}
