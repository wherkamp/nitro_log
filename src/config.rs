use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;
use std::vec::IntoIter;

use log::Level;
use log::Level::{Debug, Error, Info, Trace, Warn};
use serde::{Deserialize, Deserializer, Serialize};

use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, Visitor};

use serde_json::Value;

use crate::format::Format;
use crate::loggers::target::LoggerTarget;
use crate::{Logger, LoggerBuilders};

#[derive(Serialize, Deserialize)]
pub struct FormatConfig {
    pub format: String,
    pub placeholders: HashMap<String, Value>,
}

impl FromStr for FormatConfig {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(FormatConfig {
            format: s.to_string(),
            placeholders: Default::default(),
        })
    }
}

impl From<String> for FormatConfig {
    fn from(format: String) -> Self {
        FormatConfig {
            format,
            placeholders: Default::default(),
        }
    }
}

pub(crate) fn format_config_string_or_struct<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = ()>,
    D: Deserializer<'de>,
{
    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = ()>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<T, E>
        where
            E: serde::de::Error,
        {
            Ok(FromStr::from_str(value).unwrap())
        }

        fn visit_map<M>(self, map: M) -> Result<T, M::Error>
        where
            M: MapAccess<'de>,
        {
            Deserialize::deserialize(MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

/// Target Config
#[derive(Serialize, Deserialize)]
pub struct TargetConfig {
    /// Target Name Ex: console or file
    #[serde(rename = "type")]
    pub target_type: String,
    /// Properties. Refer to Target config struct for more information
    #[serde(default)]
    pub properties: Value,
}

/// For Loggers with modules
#[derive(Serialize, Deserialize)]
pub struct LoggerConfig {
    /// Should be a unique name for the logger
    /// This will default to module value. If module is not set, it will default to "root"
    pub logger_name: Option<String>,
    /// The target module. This should not be set if it is a root logger
    pub module: Option<String>,
    /// Levels
    #[serde(default = "default_levels")]
    pub levels: Vec<Level>,
    /// Targets
    pub targets: Vec<TargetConfig>,
    /// Format
    #[serde(deserialize_with = "format_config_string_or_struct")]
    pub format: FormatConfig,
    /// Structure Dump
    /// Dump the yaks
    #[serde(default)]
    pub structure_dump: bool,
    /// Do you want to always execute based on module parents
    /// If you have a module nitro::admin::system and nitro::admin
    /// if nitro::admin has this set to true
    /// And you grab the loggers for nitro::admin::system
    /// it will return true
    #[serde(default)]
    pub always_execute: bool,
}

fn default_levels() -> Vec<Level> {
    vec![Trace, Info, Debug, Warn, Error]
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    /// All the logger
    #[serde(default)]
    pub loggers: Vec<LoggerConfig>,
    ///Default Loggers
    pub root_loggers: Vec<LoggerConfig>,
}

pub fn create_loggers(
    config: Config,
  mut  builders:  LoggerBuilders,
) -> Result<(Vec<Logger>, Vec<Logger>), crate::Error> {
    Ok((
        create_logger(config.root_loggers.into_iter(), &mut builders)?,
        create_logger(config.loggers.into_iter(), &mut builders)?,
    ))
}

fn create_logger(
    loggers: IntoIter<LoggerConfig>,
    builders: &mut LoggerBuilders,
) -> Result<Vec<Logger>, crate::Error> {
    let mut values = Vec::new();
    for config in loggers {
        let mut logger = Logger {
            logger_name: config
                .logger_name
                .unwrap_or_else(|| config.module.clone().unwrap_or_else(|| "root".to_string())),
            module: config.module,
            levels: config.levels,
            targets: Default::default(),
            always_execute: config.always_execute,
            structure_dump: config.structure_dump,
            format: Format::new(&builders.placeholders, config.format, false)?,
        };
        for target in config.targets {
            logger
                .targets
                .push(create_target(&logger, target, builders)?);
        }
        values.push(logger);
    }
    Ok(values)
}

fn create_target(
    logger: &Logger,
    target: TargetConfig,
    builders: &mut LoggerBuilders,
) -> Result<Box<dyn LoggerTarget>, crate::Error> {
    if let Some(target_builder) = builders
        .targets
        .iter_mut()
        .find(|target_builder| target_builder.name().eq(&target.target_type))
    {
        match target_builder.build(logger, target.properties, &builders.placeholders) {
            Ok(value) => Ok(value),
            Err(error) => Err(error),
        }
    } else {
        todo!("Implement Error Handler here")
    }
}
