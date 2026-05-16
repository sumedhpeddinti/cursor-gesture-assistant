use cursor_core::{default_config_path, AppConfig, HelperCommand, HelperReply, SelectionMode, StartupMode};
use eframe::{egui, App, Frame, NativeOptions};
use std::{io::{BufRead, BufReader, Write}, net::TcpStream};

struct SettingsApp {
    config_path: std::path::PathBuf,
    config: AppConfig,
    api_key_buffer: String,
    status_message: String,
    helper_message: String,
}

impl SettingsApp {
    fn load() -> Self {
        let config_path = default_config_path();
        let config = AppConfig::load(&config_path).unwrap_or_default();
        let api_key_buffer = config.api_key.clone().unwrap_or_default();

        Self {
            config_path,
            config,
            api_key_buffer,
            status_message: "Ready".to_string(),
            helper_message: "Helper not checked yet".to_string(),
        }
    }

    fn sync_api_key(&mut self) {
        let trimmed = self.api_key_buffer.trim().to_string();
        self.config.api_key = if trimmed.is_empty() { None } else { Some(trimmed) };
    }

    fn save(&mut self) {
        self.sync_api_key();
        match self.config.save(&self.config_path) {
            Ok(()) => {
                self.status_message = format!("Saved to {}", self.config_path.display());
                match send_helper_command(self.config.helper_port, HelperCommand::UpdateConfig(self.config.clone())) {
                    Ok(HelperReply::Ack) => self.helper_message = "Helper configuration refreshed".to_string(),
                    Ok(reply) => self.helper_message = format!("Unexpected reply: {reply:?}"),
                    Err(error) => self.helper_message = format!("Saved locally, helper sync failed: {error}"),
                }
            }
            Err(error) => self.status_message = format!("Save failed: {error}"),
        }
    }

    fn ping_helper(&mut self) {
        match send_helper_command(self.config.helper_port, HelperCommand::Ping) {
            Ok(HelperReply::Pong) => self.helper_message = "Helper is alive".to_string(),
            Ok(reply) => self.helper_message = format!("Unexpected reply: {reply:?}"),
            Err(error) => self.helper_message = format!("Helper ping failed: {error}"),
        }
    }

    fn simulate_gesture(&mut self) {
        match send_helper_command(self.config.helper_port, HelperCommand::SimulateGesture) {
            Ok(HelperReply::Ack) => self.helper_message = "Gesture pipeline activated".to_string(),
            Ok(reply) => self.helper_message = format!("Unexpected reply: {reply:?}"),
            Err(error) => self.helper_message = format!("Gesture test failed: {error}"),
        }
    }
}

impl App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        let panel_fill = egui::Color32::from_rgba_unmultiplied(18, 22, 32, 190);
        let accent = egui::Color32::from_rgb(88, 172, 255);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(panel_fill).inner_margin(24.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(14.0, 14.0);
                ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(240, 245, 255));

                ui.horizontal(|ui| {
                    ui.heading("Cursor Gesture Assistant");
                    ui.label(egui::RichText::new("glassy Windows setup").color(accent));
                });

                ui.add_space(8.0);
                ui.label("API key setup, startup choice, and gesture behavior live here.");

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("API key");
                    let field = egui::TextEdit::singleline(&mut self.api_key_buffer)
                        .password(true)
                        .hint_text("Paste Gemini API key here");
                    ui.add_sized([420.0, 24.0], field);
                });

                ui.horizontal(|ui| {
                    ui.label("Startup mode");
                    ui.selectable_value(&mut self.config.startup_mode, StartupMode::Manual, "Manual");
                    ui.selectable_value(&mut self.config.startup_mode, StartupMode::Auto, "Autostart");
                });

                ui.horizontal(|ui| {
                    ui.label("Selection mode");
                    ui.selectable_value(&mut self.config.selection_mode, SelectionMode::TextFirst, "Text first");
                    ui.selectable_value(&mut self.config.selection_mode, SelectionMode::ScreenshotFallback, "Screenshot fallback");
                });

                ui.add(egui::Slider::new(&mut self.config.gesture_threshold, 1..=30).text("Gesture sensitivity"));
                ui.add(egui::Slider::new(&mut self.config.ui_opacity, 0.5..=1.0).text("UI opacity"));

                ui.checkbox(&mut self.config.no_history, "Disable local history");
                ui.horizontal(|ui| {
                    ui.label("Model");
                    ui.add_sized([220.0, 24.0], egui::TextEdit::singleline(&mut self.config.model_name));
                });

                ui.horizontal(|ui| {
                    if ui.button("Save settings").clicked() {
                        self.save();
                    }
                    if ui.button("Ping helper").clicked() {
                        self.ping_helper();
                    }
                    if ui.button("Test gesture path").clicked() {
                        self.simulate_gesture();
                    }
                });

                ui.separator();
                ui.label(&self.status_message);
                ui.label(&self.helper_message);
            });
    }
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

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_inner_size([960.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Cursor Gesture Assistant",
        options,
        Box::new(|_cc| Ok(Box::new(SettingsApp::load()))),
    )
}
