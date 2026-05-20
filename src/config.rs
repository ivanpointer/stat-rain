use crate::mapping::{MappingExpression, VisualAttribute};
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
            MappingExpression::new("cpu.normalized * 8 + 1").unwrap(),
        );
        mappings.insert(
            VisualAttribute::Density,
            MappingExpression::new("memory.normalized * 0.5 + 0.2").unwrap(),
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
}
