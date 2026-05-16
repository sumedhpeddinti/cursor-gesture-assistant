use cursor_core::{default_config_path, AppConfig, HelperCommand, HelperReply};
use std::{env, io::{BufRead, BufReader, Write}, net::TcpStream, path::PathBuf, process::Command, thread, time::Duration};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--open-settings") {
        open_settings();
        return;
    }

    let config_path = default_config_path();
    let config = AppConfig::load(&config_path).unwrap_or_default();

    if let Err(error) = send_helper_command(config.helper_port, HelperCommand::Ping) {
        eprintln!("helper not reachable yet: {error}");
        let _ = start_helper_process();
        thread::sleep(Duration::from_millis(250));
    }

    if let Ok(reply) = send_helper_command(config.helper_port, HelperCommand::Ping) {
        println!("helper handshake: {reply:?}");
    }

    println!("cursor-tray bootstrap running");
    println!("config: {}", config_path.display());
    println!("use --open-settings to launch the settings UI once the binaries are built");

    loop {
        thread::sleep(Duration::from_secs(5));
    }
}

fn open_settings() {
    let settings_exe = settings_executable_path();

    match Command::new(&settings_exe).spawn() {
        Ok(_) => println!("settings UI launched"),
        Err(error) => eprintln!("failed to launch settings UI: {error}"),
    }
}

fn start_helper_process() -> std::io::Result<()> {
    let helper_exe = sibling_executable_path("cursor-helper.exe", "cursor-helper");
    Command::new(helper_exe).spawn().map(|_| ())
}

fn settings_executable_path() -> PathBuf {
    sibling_executable_path("cursor-settings.exe", "cursor-settings")

}

fn sibling_executable_path(windows_name: &str, unix_name: &str) -> PathBuf {
    let executable_name = if cfg!(windows) { windows_name } else { unix_name };

    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(executable_name)))
        .unwrap_or_else(|| PathBuf::from(executable_name))
}

fn send_helper_command(port: u16, command: HelperCommand) -> std::io::Result<HelperReply> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    let payload = serde_json::to_string(&command)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    serde_json::from_str(response.trim())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}
