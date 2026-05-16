use cursor_core::{default_config_path, AppConfig, HelperCommand, HelperReply, HelperStatus};
use std::{io::{self, BufRead, BufReader, Write}, net::{TcpListener, TcpStream}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, thread, time::Duration};

#[derive(Debug)]
struct HelperRuntime {
    config: AppConfig,
    status: HelperStatus,
}

impl HelperRuntime {
    fn new(config: AppConfig) -> Self {
        Self {
            config,
            status: HelperStatus::default(),
        }
    }

    fn apply_config(&mut self, config: AppConfig) {
        self.status.last_action = Some("configuration updated".to_string());
        self.config = config;
        let _ = self.config.save(default_config_path());
    }

    fn handle_command(&mut self, command: HelperCommand) -> HelperReply {
        match command {
            HelperCommand::Ping => HelperReply::Pong,
            HelperCommand::GetStatus => HelperReply::Status(self.status.clone()),
            HelperCommand::UpdateConfig(config) => {
                self.apply_config(config);
                HelperReply::Ack
            }
            HelperCommand::SimulateGesture => {
                self.status.waiting_for_gesture = false;
                self.status.last_action = Some("gesture detected; capture pipeline armed".to_string());
                HelperReply::Ack
            }
            HelperCommand::Shutdown => {
                self.status.running = false;
                self.status.last_action = Some("shutdown requested".to_string());
                HelperReply::Ack
            }
        }
    }
}

fn main() -> io::Result<()> {
    let config_path = default_config_path();
    let config = AppConfig::load(&config_path).unwrap_or_default();
    let listener = TcpListener::bind(("127.0.0.1", config.helper_port))?;
    listener.set_nonblocking(true)?;
    let runtime = Arc::new(Mutex::new(HelperRuntime::new(config.clone())));
    let running = Arc::new(AtomicBool::new(true));

    println!("cursor-helper listening on 127.0.0.1:{}", config.helper_port);

    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let runtime = Arc::clone(&runtime);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    if let Err(error) = handle_client(stream, runtime, running) {
                        eprintln!("helper client error: {error}");
                    }
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                eprintln!("helper accept error: {error}");
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    Ok(())
}

fn handle_client(stream: TcpStream, runtime: Arc<Mutex<HelperRuntime>>, running: Arc<AtomicBool>) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let mut request = String::new();
    reader.read_line(&mut request)?;

    if request.trim().is_empty() {
        return Ok(());
    }

    let command: HelperCommand = serde_json::from_str(request.trim())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let response = {
        let mut state = runtime.lock().expect("helper runtime poisoned");
        let reply = state.handle_command(command);
        if !state.status.running {
            running.store(false, Ordering::Relaxed);
        }
        reply
    };

    let payload = serde_json::to_string(&response)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    writer.write_all(payload.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
