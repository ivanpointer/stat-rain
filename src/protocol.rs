use anyhow::{bail, Result};

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
    },
    TextInjection {
        text: String,
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
            Self::MetricStale { name } => {
                output.push(2);
                write_string(output, name);
            }
            Self::TextInjection { text } => {
                output.push(3);
                write_string(output, text);
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
            2 => Ok(Self::MetricStale {
                name: cursor.read_string()?,
            }),
            3 => Ok(Self::TextInjection {
                text: cursor.read_string()?,
            }),
            kind => bail!("unsupported protocol message kind: {kind}"),
        }
    }
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
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }
}
