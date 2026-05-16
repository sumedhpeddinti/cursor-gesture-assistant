use crate::{AppConfig, HelperCommand, HelperReply, HelperStatus, SelectionMode, StartupMode};

pub fn describe_command(command: &HelperCommand) -> &'static str {
    match command {
        HelperCommand::Ping => "ping",
        HelperCommand::GetStatus => "status",
        HelperCommand::UpdateConfig(_) => "update-config",
        HelperCommand::SimulateGesture => "simulate-gesture",
        HelperCommand::Shutdown => "shutdown",
    }
}

pub fn default_status_for_selection_mode(selection_mode: &SelectionMode) -> HelperStatus {
    HelperStatus {
        running: true,
        waiting_for_gesture: matches!(selection_mode, SelectionMode::TextFirst),
        last_action: None,
    }
}

pub fn summarize_config(config: &AppConfig) -> String {
    format!(
        "mode={:?}; history={}; port={}; threshold={}; selection={:?}; model={}",
        config.startup_mode,
        config.no_history,
        config.helper_port,
        config.gesture_threshold,
        config.selection_mode,
        config.model_name,
    )
}

pub fn ok_reply() -> HelperReply {
    HelperReply::Ack
}
