use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use crate::metrics::MetricRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualAttribute {
    Speed,
    Density,
    ColorHotness,
    Brightness,
    FadeLength,
    GlyphChurn,
    MessageRevealIntensity,
    EmberDensity,
    EmberBrightness,
    EmberColorHotness,
    EmberFadeLength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingExpression(String);

impl MappingExpression {
    pub fn new(value: impl Into<String>) -> Result<Self, MappingError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(MappingError::EmptyExpression);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn evaluate(&self, metrics: &MetricRegistry) -> Result<f64, MappingError> {
        Parser::new(&self.0, metrics).parse()
    }

    pub fn referenced_metrics(&self) -> Result<BTreeSet<String>, MappingError> {
        ReferenceParser::new(&self.0).parse()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    EmptyExpression,
    ExpectedNumberOrMetric,
    UnexpectedToken(String),
    UnknownMetricField(String),
}

struct ReferenceParser<'a> {
    input: &'a str,
    offset: usize,
    metrics: BTreeSet<String>,
}

impl<'a> ReferenceParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
            metrics: BTreeSet::new(),
        }
    }

    fn parse(mut self) -> Result<BTreeSet<String>, MappingError> {
        while self.offset < self.input.len() {
            self.skip_whitespace();
            if self
                .peek()
                .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
            {
                self.parse_metric_reference()?;
            } else if let Some(value) = self.peek() {
                self.offset += value.len_utf8();
            }
        }
        Ok(self.metrics)
    }

    fn parse_metric_reference(&mut self) -> Result<(), MappingError> {
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|value| value.is_ascii_alphanumeric() || value == '_' || value == '.')
        {
            self.offset += 1;
        }
        let field = &self.input[start..self.offset];
        let Some((metric, value_kind)) = field.rsplit_once('.') else {
            return Err(MappingError::UnknownMetricField(field.to_string()));
        };
        match value_kind {
            "raw" | "normalized" => {
                self.metrics.insert(metric.to_string());
                Ok(())
            }
            _ => Err(MappingError::UnknownMetricField(field.to_string())),
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|value| value.is_whitespace()) {
            self.offset += 1;
        }
    }
}

impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExpression => write!(f, "mapping expression cannot be empty"),
            Self::ExpectedNumberOrMetric => write!(f, "expected number or metric field"),
            Self::UnexpectedToken(token) => write!(f, "unexpected token: {token}"),
            Self::UnknownMetricField(field) => write!(f, "unknown metric field: {field}"),
        }
    }
}

impl std::error::Error for MappingError {}

impl FromStr for VisualAttribute {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "speed" => Ok(Self::Speed),
            "density" => Ok(Self::Density),
            "color_hotness" => Ok(Self::ColorHotness),
            "brightness" => Ok(Self::Brightness),
            "fade_length" => Ok(Self::FadeLength),
            "glyph_churn" => Ok(Self::GlyphChurn),
            "message_reveal_intensity" => Ok(Self::MessageRevealIntensity),
            "ember_density" => Ok(Self::EmberDensity),
            "ember_brightness" => Ok(Self::EmberBrightness),
            "ember_color_hotness" => Ok(Self::EmberColorHotness),
            "ember_fade_length" => Ok(Self::EmberFadeLength),
            _ => Err(format!("unknown visual attribute: {value}")),
        }
    }
}

struct Parser<'a> {
    input: &'a str,
    offset: usize,
    metrics: &'a MetricRegistry,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, metrics: &'a MetricRegistry) -> Self {
        Self {
            input,
            offset: 0,
            metrics,
        }
    }

    fn parse(mut self) -> Result<f64, MappingError> {
        let value = self.parse_expression()?;
        self.skip_whitespace();
        if self.offset < self.input.len() {
            return Err(MappingError::UnexpectedToken(
                self.input[self.offset..].to_string(),
            ));
        }
        Ok(value)
    }

    fn parse_expression(&mut self) -> Result<f64, MappingError> {
        let mut value = self.parse_term()?;

        loop {
            self.skip_whitespace();
            if self.consume('+') {
                value += self.parse_term()?;
            } else if self.consume('-') {
                value -= self.parse_term()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, MappingError> {
        let mut value = self.parse_factor()?;

        loop {
            self.skip_whitespace();
            if self.consume('*') {
                value *= self.parse_factor()?;
            } else if self.consume('/') {
                value /= self.parse_factor()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_factor(&mut self) -> Result<f64, MappingError> {
        self.skip_whitespace();

        if self.consume('-') {
            return Ok(-self.parse_factor()?);
        }

        if self.peek().is_some_and(|value| value.is_ascii_digit()) {
            return self.parse_number();
        }

        if self
            .peek()
            .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
        {
            return self.parse_metric_field();
        }

        Err(MappingError::ExpectedNumberOrMetric)
    }

    fn parse_number(&mut self) -> Result<f64, MappingError> {
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|value| value.is_ascii_digit() || value == '.')
        {
            self.offset += 1;
        }
        self.input[start..self.offset]
            .parse::<f64>()
            .map_err(|_| MappingError::ExpectedNumberOrMetric)
    }

    fn parse_metric_field(&mut self) -> Result<f64, MappingError> {
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|value| value.is_ascii_alphanumeric() || value == '_' || value == '.')
        {
            self.offset += 1;
        }

        let field = &self.input[start..self.offset];
        let Some((metric, value_kind)) = field.rsplit_once('.') else {
            return Err(MappingError::UnknownMetricField(field.to_string()));
        };

        let Some(value) = self.metrics.get(metric) else {
            return Ok(0.0);
        };

        match value_kind {
            "raw" => Ok(value.raw.unwrap_or(0.0)),
            "normalized" => Ok(value.normalized.unwrap_or(0.0)),
            _ => Err(MappingError::UnknownMetricField(field.to_string())),
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.offset += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|value| value.is_whitespace()) {
            self.offset += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricRegistry, MetricValue};

    #[test]
    fn evaluates_arithmetic_with_normalized_metric() {
        let mut metrics = MetricRegistry::default();
        metrics.set("cpu", MetricValue::new(Some(42.0), Some(0.5)));
        let expression = MappingExpression::new("cpu.normalized * 8 + 1").unwrap();

        let value = expression.evaluate(&metrics).unwrap();

        assert_eq!(value, 5.0);
    }

    #[test]
    fn evaluates_arithmetic_with_raw_metric() {
        let mut metrics = MetricRegistry::default();
        metrics.set("thermal_zone", MetricValue::new(Some(65.0), None));
        let expression = MappingExpression::new("thermal_zone.raw / 100").unwrap();

        let value = expression.evaluate(&metrics).unwrap();

        assert_eq!(value, 0.65);
    }

    #[test]
    fn reports_referenced_metric_names() {
        let expression =
            MappingExpression::new("cpu.normalized * 4 + thermal_zone.raw / 100").unwrap();

        let references = expression.referenced_metrics().unwrap();

        assert_eq!(
            references,
            BTreeSet::from(["cpu".to_string(), "thermal_zone".to_string()])
        );
    }

    #[test]
    fn reports_reference_errors_for_unknown_fields() {
        let expression = MappingExpression::new("cpu.percent").unwrap();

        let error = expression.referenced_metrics().unwrap_err();

        assert_eq!(
            error,
            MappingError::UnknownMetricField("cpu.percent".to_string())
        );
    }
}
