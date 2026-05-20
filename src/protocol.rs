use crate::message::MessageClass;
use anyhow::{bail, Result};
use std::io::{Read, Write};

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolMessage {
    MetricUpdate {
        name: String,
        raw: Option<f64>,
        normalized: Option<f64>,
    },
    MetricStale {
        name: String,
        reason: Option<String>,
    },
    MetricError {
        name: String,
        reason: Option<String>,
    },
    MetricStatusClear {
        name: String,
    },
    TextInjection {
        text: String,
        class: MessageClass,
        ttl_ms: Option<u64>,
    },
}

impl ProtocolMessage {
    pub fn encode(&self, output: &mut Vec<u8>) {
        output.push(PROTOCOL_VERSION);
        match self {
            Self::MetricUpdate {
                name,
                raw,
                normalized,
            } => {
                output.push(1);
                write_string(output, name);
                write_optional_f64(output, *raw);
                write_optional_f64(output, *normalized);
            }
            Self::MetricStale { name, reason } => {
                output.push(2);
                write_string(output, name);
                write_optional_string(output, reason.as_deref());
            }
            Self::TextInjection {
                text,
                class,
                ttl_ms,
            } => {
                output.push(3);
                write_string(output, text);
                output.push(class.to_wire());
                write_optional_u64(output, *ttl_ms);
            }
            Self::MetricError { name, reason } => {
                output.push(4);
                write_string(output, name);
                write_optional_string(output, reason.as_deref());
            }
            Self::MetricStatusClear { name } => {
                output.push(5);
                write_string(output, name);
            }
        }
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < 2 {
            bail!("protocol message too short");
        }
        if input[0] != PROTOCOL_VERSION {
            bail!("unsupported protocol version: {}", input[0]);
        }

        let mut cursor = Cursor::new(&input[2..]);
        match input[1] {
            1 => Ok(Self::MetricUpdate {
                name: cursor.read_string()?,
                raw: cursor.read_optional_f64()?,
                normalized: cursor.read_optional_f64()?,
            }),
            2 => {
                let name = cursor.read_string()?;
                let reason = if cursor.remaining() == 0 {
                    None
                } else {
                    cursor.read_optional_string()?
                };
                Ok(Self::MetricStale { name, reason })
            }
            3 => {
                let text = cursor.read_string()?;
                let class = if cursor.remaining() == 0 {
                    MessageClass::Info
                } else {
                    MessageClass::from_wire(cursor.read_u8()?)?
                };
                let ttl_ms = if cursor.remaining() == 0 {
                    None
                } else {
                    cursor.read_optional_u64()?
                };
                Ok(Self::TextInjection {
                    text,
                    class,
                    ttl_ms,
                })
            }
            4 => {
                let name = cursor.read_string()?;
                let reason = cursor.read_optional_string()?;
                Ok(Self::MetricError { name, reason })
            }
            5 => Ok(Self::MetricStatusClear {
                name: cursor.read_string()?,
            }),
            kind => bail!("unsupported protocol message kind: {kind}"),
        }
    }
}

pub fn write_framed_message(mut output: impl Write, message: &ProtocolMessage) -> Result<()> {
    let mut payload = Vec::new();
    message.encode(&mut payload);
    let len = u32::try_from(payload.len())?;
    output.write_all(&len.to_le_bytes())?;
    output.write_all(&payload)?;
    Ok(())
}

pub fn read_framed_message(mut input: impl Read) -> Result<ProtocolMessage> {
    let mut len_bytes = [0_u8; 4];
    input.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    let mut payload = vec![0_u8; len];
    input.read_exact(&mut payload)?;
    ProtocolMessage::decode(&payload)
}

fn write_string(output: &mut Vec<u8>, value: &str) {
    let len = value.len() as u16;
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn write_optional_f64(output: &mut Vec<u8>, value: Option<f64>) {
    match value {
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value.to_le_bytes());
        }
        None => output.push(0),
    }
}

fn write_optional_u64(output: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value.to_le_bytes());
        }
        None => output.push(0),
    }
}

fn write_optional_string(output: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            output.push(1);
            write_string(output, value);
        }
        None => output.push(0),
    }
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn read_string(&mut self) -> Result<String> {
        let len_bytes = self.read_exact(2)?;
        let len = u16::from_le_bytes([len_bytes[0], len_bytes[1]]) as usize;
        let bytes = self.read_exact(len)?;
        Ok(String::from_utf8(bytes.to_vec())?)
    }

    fn read_optional_f64(&mut self) -> Result<Option<f64>> {
        let tag = self.read_exact(1)?[0];
        match tag {
            0 => Ok(None),
            1 => {
                let bytes = self.read_exact(8)?;
                Ok(Some(f64::from_le_bytes(bytes.try_into().unwrap())))
            }
            _ => bail!("invalid optional f64 tag: {tag}"),
        }
    }

    fn read_optional_u64(&mut self) -> Result<Option<u64>> {
        let tag = self.read_exact(1)?[0];
        match tag {
            0 => Ok(None),
            1 => {
                let bytes = self.read_exact(8)?;
                Ok(Some(u64::from_le_bytes(bytes.try_into().unwrap())))
            }
            _ => bail!("invalid optional u64 tag: {tag}"),
        }
    }

    fn read_optional_string(&mut self) -> Result<Option<String>> {
        let tag = self.read_exact(1)?[0];
        match tag {
            0 => Ok(None),
            1 => Ok(Some(self.read_string()?)),
            _ => bail!("invalid optional string tag: {tag}"),
        }
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_exact(1)?[0])
    }

    fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.offset)
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.offset + len;
        if end > self.input.len() {
            bail!("protocol message truncated");
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageClass;

    #[test]
    fn round_trips_metric_update() {
        let message = ProtocolMessage::MetricUpdate {
            name: "cpu".to_string(),
            raw: Some(42.0),
            normalized: Some(0.42),
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }

    #[test]
    fn round_trips_text_injection() {
        let message = ProtocolMessage::TextInjection {
            text: "SYSTEM OK".to_string(),
            class: MessageClass::Warning,
            ttl_ms: Some(10_000),
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }

    #[test]
    fn decodes_legacy_text_injection_as_info() {
        let mut encoded = vec![PROTOCOL_VERSION, 3];
        write_string(&mut encoded, "SYSTEM OK");

        assert_eq!(
            ProtocolMessage::decode(&encoded).unwrap(),
            ProtocolMessage::TextInjection {
                text: "SYSTEM OK".to_string(),
                class: MessageClass::Info,
                ttl_ms: None,
            }
        );
    }

    #[test]
    fn round_trips_metric_stale_with_reason() {
        let message = ProtocolMessage::MetricStale {
            name: "thermal_zone".to_string(),
            reason: Some("sensor timeout".to_string()),
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }

    #[test]
    fn round_trips_metric_error_with_reason() {
        let message = ProtocolMessage::MetricError {
            name: "thermal_zone".to_string(),
            reason: Some("read failed".to_string()),
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }

    #[test]
    fn round_trips_metric_status_clear() {
        let message = ProtocolMessage::MetricStatusClear {
            name: "thermal_zone".to_string(),
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }

    #[test]
    fn round_trips_framed_message() {
        let message = ProtocolMessage::MetricUpdate {
            name: "cpu".to_string(),
            raw: None,
            normalized: Some(0.99),
        };
        let mut stream = Vec::new();

        write_framed_message(&mut stream, &message).unwrap();

        assert_eq!(
            read_framed_message(&mut stream.as_slice()).unwrap(),
            message
        );
    }

    #[test]
    fn rejects_truncated_framed_message() {
        let mut stream = [8_u8, 0, 0, 0, PROTOCOL_VERSION, 1].as_slice();

        assert!(read_framed_message(&mut stream).is_err());
    }
}
