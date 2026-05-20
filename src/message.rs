use anyhow::{bail, Result};
use clap::ValueEnum;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MessageClass {
    Info,
    Success,
    Warning,
    Error,
}

impl MessageClass {
    pub fn to_wire(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Success => 1,
            Self::Warning => 2,
            Self::Error => 3,
        }
    }

    pub fn from_wire(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Info),
            1 => Ok(Self::Success),
            2 => Ok(Self::Warning),
            3 => Ok(Self::Error),
            _ => bail!("invalid message class: {value}"),
        }
    }

    pub fn color_hotness_bucket(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Success => 0,
            Self::Warning => 190,
            Self::Error => 255,
        }
    }

    pub fn brightness_floor(self) -> f64 {
        match self {
            Self::Info => 0.72,
            Self::Success => 0.76,
            Self::Warning => 0.82,
            Self::Error => 0.88,
        }
    }

    pub fn glitch_boost(self) -> f64 {
        match self {
            Self::Info => 1.0,
            Self::Success => 0.85,
            Self::Warning => 1.25,
            Self::Error => 1.55,
        }
    }

    pub fn color_bucket(self) -> u8 {
        match self {
            Self::Info => 1,
            Self::Success => 2,
            Self::Warning => 3,
            Self::Error => 4,
        }
    }
}

impl Default for MessageClass {
    fn default() -> Self {
        Self::Info
    }
}

impl fmt::Display for MessageClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        })
    }
}

impl FromStr for MessageClass {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "info" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            _ => bail!("invalid message class: {value}"),
        }
    }
}
