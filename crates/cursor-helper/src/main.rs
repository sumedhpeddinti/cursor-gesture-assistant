use cursor_core::{default_config_path, AppConfig, HelperCommand, HelperReply, HelperStatus};
use once_cell::sync::OnceCell;
use std::{io::{self, BufRead, BufReader, Write}, net::{TcpListener, TcpStream}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, thread, time::{Duration, Instant}};
use std::sync::Mutex as StdMutex;
use windows::Win32::Foundation::{LRESULT, WPARAM, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, CallNextHookEx, UnhookWindowsHookEx, DispatchMessageW, GetMessageW, TranslateMessage, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, HC_ACTION, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE};

static HOOK_PORT: OnceCell<u16> = OnceCell::new();
static HOOK_THRESHOLD: OnceCell<u8> = OnceCell::new();

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
    // start the global mouse hook thread to detect left-click + wiggle gestures
    start_mouse_hook_thread(config.helper_port, config.gesture_threshold);

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

fn start_mouse_hook_thread(port: u16, threshold: u8) {
    HOOK_PORT.set(port).ok();
    HOOK_THRESHOLD.set(threshold).ok();

    // spawn a thread which installs a low-level mouse hook and runs a message loop
    thread::spawn(move || unsafe {
        use std::ptr::null_mut;

        static MOUSE_STATE: OnceCell<StdMutex<MouseState>> = OnceCell::new();
        MOUSE_STATE.get_or_init(|| StdMutex::new(MouseState::default()));

        extern "system" fn low_level_mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
            if n_code == HC_ACTION as i32 {
                unsafe {
                    let port = HOOK_PORT.get().copied().unwrap_or(48881);
                    let threshold = HOOK_THRESHOLD.get().copied().unwrap_or(12);
                    let state = MOUSE_STATE.get().expect("mouse state");
                    let mut s = state.lock().unwrap();

                    let ms_ptr = l_param.0 as *const MSLLHOOKSTRUCT;
                    if !ms_ptr.is_null() {
                        let ms = *ms_ptr;
                        match w_param.0 as u32 {
                            WM_LBUTTONDOWN => {
                                s.holding = true;
                                s.start = Instant::now();
                                s.last_x = ms.pt.x;
                                s.last_y = ms.pt.y;
                                s.direction = 0;
                                s.wiggles = 0;
                            }
                            WM_MOUSEMOVE => {
                                if s.holding {
                                    let dx = ms.pt.x - s.last_x;
                                    let dy = ms.pt.y - s.last_y;
                                    let dist = (dx*dx + dy*dy) as f64;
                                    let dir = if dx.abs() > dy.abs() { if dx>0 {1} else {-1} } else { if dy>0 {1} else {-1} };
                                    if s.direction == 0 {
                                        s.direction = dir;
                                    } else if dir != s.direction {
                                        s.wiggles += 1;
                                        s.direction = dir;
                                    }
                                    s.last_x = ms.pt.x;
                                    s.last_y = ms.pt.y;
                                    if s.wiggles >= 3 && dist > (threshold as i32 * threshold as i32) as f64 {
                                        // trigger gesture
                                        let _ = send_local_simulate(port);
                                        s.holding = false;
                                    }
                                }
                            }
                            WM_LBUTTONUP => {
                                s.holding = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
        }

        // install hook
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), None, 0);
        if hook.is_invalid() {
            eprintln!("failed to install mouse hook");
            return;
        }

        // message loop to keep the hook alive
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(hook);
    });
}

#[derive(Default)]
struct MouseState {
    holding: bool,
    start: Instant,
    last_x: i32,
    last_y: i32,
    direction: i32,
    wiggles: u8,
}

fn send_local_simulate(port: u16) -> io::Result<()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    let payload = serde_json::to_string(&HelperCommand::SimulateGesture)
        .map_err(|error| io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}
