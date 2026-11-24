//! Console-owned Files contract. No caller URL, key, presigned upload or end-user namespace.
use super::input;
use crate::error::GatewayError;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

pub const MAX_UPLOAD: usize = 8 * 1024 * 1024;
pub const PAGE_SIZE: usize = 25;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum Operation {
    List {
        cursor: Option<String>,
    },
    Get {
        id: String,
    },
    Delete {
        id: String,
    },
    Upload {
        filename: String,
        purpose: Purpose,
        data: String,
    },
    #[serde(skip)]
    Presigned {
        metadata: super::file_upload::Upload,
        bytes: super::file_upload::UploadBody,
    },
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Purpose {
    Input,
    Reference,
}
impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Reference => "reference",
        }
    }
}

pub enum Prepared {
    Presigned {
        metadata: super::file_upload::Upload,
        bytes: super::file_upload::UploadBody,
    },
    List(Option<String>),
    Get(String),
    Delete(String),
    Upload {
        filename: String,
        purpose: Purpose,
        bytes: Vec<u8>,
    },
}
impl Operation {
    pub fn prepare(self) -> Result<Prepared, GatewayError> {
        match self {
            Self::Presigned { metadata, bytes } => {
                metadata.validate()?;
                if bytes.size != metadata.size_bytes {
                    return Err(invalid());
                }
                Ok(Prepared::Presigned { metadata, bytes })
            }
            Self::List { cursor } => {
                if cursor.as_deref().is_some_and(|v| !valid_cursor(v)) {
                    return Err(invalid());
                }
                Ok(Prepared::List(cursor))
            }
            Self::Get { id } => Ok(Prepared::Get(file_id(&id)?)),
            Self::Delete { id } => Ok(Prepared::Delete(file_id(&id)?)),
            Self::Upload {
                filename,
                purpose,
                data,
            } => {
                // Filename is one MIME parameter, never a local path or another header.
                if filename.is_empty()
                    || filename.len() > 255
                    || filename
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '\\' | '"'))
                    || data.len() > MAX_UPLOAD.div_ceil(3) * 4
                {
                    return Err(invalid());
                }
                let bytes = STANDARD.decode(data).map_err(|_| invalid())?;
                if bytes.is_empty() || bytes.len() > MAX_UPLOAD {
                    return Err(invalid());
                }
                Ok(Prepared::Upload {
                    filename,
                    purpose,
                    bytes,
                })
            }
        }
    }
}
pub fn file_id(value: &str) -> Result<String, GatewayError> {
    if value.len() != 36 {
        return Err(invalid());
    }
    uuid::Uuid::parse_str(value)
        .map(|id| id.to_string())
        .map_err(|_| invalid())
}
pub fn valid_cursor(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && value.bytes().all(|b| b.is_ascii_graphic())
}
pub fn invalid() -> GatewayError {
    input(
        "luma_files_input",
        "Luma Files 参数无效；内联上传限 8 MiB，签名上传限 5 GiB，文件 ID 必须是 UUID。",
    )
}
