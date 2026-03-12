use serde::{Deserialize, Serialize};

/// A request sent from the CLI client to the daemon over the Unix socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub command: String,
    #[serde(default)]
    pub args: String,
    /// Output format (text or json)
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "text".to_string()
}

/// A response sent from the daemon back to the CLI client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Response {
    pub fn ok(output: String) -> Self {
        Response {
            status: "ok".to_string(),
            output: Some(output),
            message: None,
        }
    }

    pub fn error(message: String) -> Self {
        Response {
            status: "error".to_string(),
            output: None,
            message: Some(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_deserialize() {
        let json = r#"{"command": "ls", "args": "functions"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.command, "ls");
        assert_eq!(req.args, "functions");
        assert_eq!(req.format, "text");
    }

    #[test]
    fn test_request_deserialize_with_format() {
        let json = r#"{"command": "show", "args": "Foo", "format": "json"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.command, "show");
        assert_eq!(req.args, "Foo");
        assert_eq!(req.format, "json");
    }

    #[test]
    fn test_request_deserialize_no_args() {
        let json = r#"{"command": "ping"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.command, "ping");
        assert_eq!(req.args, "");
    }

    #[test]
    fn test_response_ok_serialize() {
        let resp = Response::ok("hello".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"output\":\"hello\""));
        assert!(!json.contains("message"));
    }

    #[test]
    fn test_response_error_serialize() {
        let resp = Response::error("bad".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"error\""));
        assert!(json.contains("\"message\":\"bad\""));
        assert!(!json.contains("output"));
    }
}
