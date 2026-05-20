use crate::effect::EffectState;
use crate::mapping::{MappingExpression, VisualAttribute};
use crate::metrics::MetricRegistry;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub active_profile: String,
    pub mappings: BTreeMap<VisualAttribute, MappingExpression>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut mappings = BTreeMap::new();
        mappings.insert(
            VisualAttribute::Speed,
            MappingExpression::new("cpu.normalized * 4 + 0.35").unwrap(),
        );
        mappings.insert(
            VisualAttribute::Density,
            MappingExpression::new("memory.normalized * 0.18 + 0.72").unwrap(),
        );
        mappings.insert(
            VisualAttribute::ColorHotness,
            MappingExpression::new("cpu.normalized").unwrap(),
        );

        Self {
            active_profile: "default".to_string(),
            mappings,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct TomlConfig {
    profiles: Option<BTreeMap<String, TomlProfile>>,
}

#[derive(Debug, Deserialize, Default)]
struct TomlProfile {
    map: Option<BTreeMap<String, String>>,
}

impl AppConfig {
    pub fn from_toml_profile(input: &str, profile: &str) -> Result<Self> {
        let parsed: TomlConfig = toml::from_str(input).context("failed to parse config TOML")?;
        let mut config = Self {
            active_profile: profile.to_string(),
            ..Self::default()
        };

        let Some(profiles) = parsed.profiles else {
            return Ok(config);
        };

        let Some(profile_config) = profiles.get(profile) else {
            bail!("profile not found: {profile}");
        };

        if let Some(map) = &profile_config.map {
            for (attribute, expression) in map {
                config.set_mapping(attribute, expression)?;
            }
        }

        Ok(config)
    }

    pub fn apply_mapping_override(&mut self, override_value: &str) -> Result<()> {
        let Some((attribute, expression)) = override_value.split_once('=') else {
            bail!("mapping override must use attribute=expression");
        };
        self.set_mapping(attribute.trim(), expression.trim())
    }

    pub fn evaluate_effect_state(&self, metrics: &MetricRegistry) -> Result<EffectState> {
        let mut state = EffectState::default();

        for (attribute, expression) in &self.mappings {
            let value = expression.evaluate(metrics)?;
            match attribute {
                VisualAttribute::Speed => state.speed = value,
                VisualAttribute::Density => state.density = value,
                VisualAttribute::ColorHotness => state.color_hotness = value,
                VisualAttribute::Brightness => state.brightness = value,
                VisualAttribute::FadeLength => state.fade_length = value,
                VisualAttribute::GlyphChurn => state.glyph_churn = value,
                VisualAttribute::MessageRevealIntensity => state.message_reveal_intensity = value,
            }
        }

        Ok(state)
    }

    fn set_mapping(&mut self, attribute: &str, expression: &str) -> Result<()> {
        let attribute = attribute
            .parse::<VisualAttribute>()
            .map_err(|message| anyhow::anyhow!(message))?;
        let expression = MappingExpression::new(expression)?;
        self.mappings.insert(attribute, expression);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricRegistry, MetricValue};

    #[test]
    fn loads_selected_profile_mapping() {
        let config = AppConfig::from_toml_profile(
            r#"
            [profiles.default.map]
            speed = "cpu.normalized * 2"
            brightness = "memory.normalized"
            "#,
            "default",
        )
        .unwrap();

        assert_eq!(config.active_profile, "default");
        assert_eq!(
            config
                .mappings
                .get(&VisualAttribute::Speed)
                .unwrap()
                .as_str(),
            "cpu.normalized * 2"
        );
        assert_eq!(
            config
                .mappings
                .get(&VisualAttribute::Brightness)
                .unwrap()
                .as_str(),
            "memory.normalized"
        );
    }

    #[test]
    fn applies_cli_mapping_override() {
        let mut config = AppConfig::default();

        config
            .apply_mapping_override("glyph_churn=cpu.normalized * 4")
            .unwrap();

        assert_eq!(
            config
                .mappings
                .get(&VisualAttribute::GlyphChurn)
                .unwrap()
                .as_str(),
            "cpu.normalized * 4"
        );
    }

    #[test]
    fn converts_mappings_to_effect_state() {
        let config = AppConfig::from_toml_profile(
            r#"
            [profiles.default.map]
            speed = "cpu.normalized * 8 + 1"
            color_hotness = "thermal_zone.raw / 100"
            brightness = "0.75"
            "#,
            "default",
        )
        .unwrap();
        let mut metrics = MetricRegistry::default();
        metrics.set("cpu", MetricValue::new(None, Some(0.5)));
        metrics.set("thermal_zone", MetricValue::new(Some(70.0), None));

        let state = config.evaluate_effect_state(&metrics).unwrap();

        assert_eq!(state.speed, 5.0);
        assert_eq!(state.color_hotness, 0.7);
        assert_eq!(state.brightness, 0.75);
    }

    #[test]
    fn default_speed_mapping_has_slower_baseline() {
        let config = AppConfig::default();
        let mut metrics = MetricRegistry::default();
        metrics.set("cpu", MetricValue::new(None, Some(0.0)));

        let state = config.evaluate_effect_state(&metrics).unwrap();

        assert_eq!(state.speed, 0.35);
    }

    #[test]
    fn default_density_mapping_has_fuller_baseline() {
        let config = AppConfig::default();
        let mut metrics = MetricRegistry::default();
        metrics.set("memory", MetricValue::new(None, Some(0.0)));

        let state = config.evaluate_effect_state(&metrics).unwrap();

        assert_eq!(state.density, 0.72);
    }
}
