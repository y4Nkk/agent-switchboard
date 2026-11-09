use super::process;
use asb_core::contracts::{AppKind, LocalizedMessage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SoftwareError {
    Operation(String),
    ProcessInUse { app: AppKind, pid: u32 },
}

impl SoftwareError {
    pub fn into_message(self) -> LocalizedMessage {
        match self {
            Self::Operation(detail) => {
                let detail = process::scrub(&detail);
                LocalizedMessage::new("software.operationFailed", serde_json::json!({ "detail": &detail }), detail)
            }
            Self::ProcessInUse { app, pid } => LocalizedMessage::new(
                "software.processInUse", serde_json::json!({ "app": app.label(), "pid": pid }),
                format!("Process {pid} is using the {} installation", app.label()),
            ),
        }
    }
}

impl From<String> for SoftwareError {
    fn from(detail: String) -> Self { Self::Operation(detail) }
}

impl From<&str> for SoftwareError {
    fn from(detail: &str) -> Self { Self::Operation(detail.into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupancy_errors_keep_their_translation_and_parameters_at_the_command_boundary() {
        for app in [AppKind::Codex, AppKind::Claude] {
            let error = crate::client_software::error(SoftwareError::ProcessInUse { app, pid: 42 });
            let value = serde_json::to_value(error).unwrap();
            assert_eq!(value["code"], "client-software-operation");
            assert_eq!(value["messageKey"], "software.processInUse");
            assert_eq!(value["params"], serde_json::json!({ "app": app.label(), "pid": 42 }));
            assert!(value["params"].get("detail").is_none());
        }
    }
}
