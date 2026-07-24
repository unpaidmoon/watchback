fn main() {
    match owstats_desktop_lib::parse_startup_mode(std::env::args_os()) {
        Ok(owstats_desktop_lib::StartupMode::Desktop) => owstats_desktop_lib::run(),
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
        Err(_) => std::process::exit(2),
    }
}
