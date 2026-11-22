//! Small, bounded diagnostic settings; gameplay commands keep their usual timing rules.

use serde::Serialize;

use crate::{mission::Config, physics::STEP_SECONDS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Parameter {
    DeadlineSeconds,
    ReloadSeconds,
    SensorProcessingSeconds,
    ReactionSeconds,
    SafetyRadiusM,
    HoldSeconds,
    RecoverySeconds,
}

impl Parameter {
    pub const ALL: [Self; 7] = [
        Self::DeadlineSeconds,
        Self::ReloadSeconds,
        Self::SensorProcessingSeconds,
        Self::ReactionSeconds,
        Self::SafetyRadiusM,
        Self::HoldSeconds,
        Self::RecoverySeconds,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::DeadlineSeconds => "deadline_seconds",
            Self::ReloadSeconds => "reload_seconds",
            Self::SensorProcessingSeconds => "sensor_processing_seconds",
            Self::ReactionSeconds => "reaction_seconds",
            Self::SafetyRadiusM => "safety_radius_m",
            Self::HoldSeconds => "hold_seconds",
            Self::RecoverySeconds => "recovery_seconds",
        }
    }

    pub fn named(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|p| p.name() == name)
            .ok_or_else(|| format!("Unknown parameter: {name}"))
    }

    pub fn initial_only(self) -> bool {
        matches!(
            self,
            Self::DeadlineSeconds | Self::ReloadSeconds | Self::SensorProcessingSeconds
        )
    }

    pub fn value(self, config: &Config) -> f64 {
        match self {
            Self::DeadlineSeconds => config.deadline_tick as f64 * STEP_SECONDS,
            Self::ReloadSeconds => config.reload_ticks as f64 * STEP_SECONDS,
            Self::SensorProcessingSeconds => config.sensor_processing_ticks as f64 * STEP_SECONDS,
            Self::ReactionSeconds => config.policy.reaction_seconds,
            Self::SafetyRadiusM => config.policy.safety_radius,
            Self::HoldSeconds => config.policy.hold_seconds,
            Self::RecoverySeconds => config.policy.recovery_seconds,
        }
    }

    pub fn apply(self, config: &mut Config, value: f64) -> Result<(), String> {
        // Bounds cap forecast work and keep diagnostics useful at this scenario's scale.
        let (min, max) = match self {
            Self::DeadlineSeconds => (STEP_SECONDS, 7200.0),
            Self::ReloadSeconds => (STEP_SECONDS, 600.0),
            Self::SensorProcessingSeconds => (0.0, 120.0),
            Self::ReactionSeconds => (0.0, 600.0),
            Self::SafetyRadiusM => (0.0, 2_000_000.0),
            Self::HoldSeconds => (0.0, 120.0),
            Self::RecoverySeconds => (STEP_SECONDS, 1200.0),
        };
        if !value.is_finite() || !(min..=max).contains(&value) {
            return Err(format!(
                "{} must be finite and between {min} and {max}.",
                self.name()
            ));
        }
        if self.initial_only() && (value / STEP_SECONDS).fract() != 0.0 {
            return Err(format!(
                "{} must be a multiple of {STEP_SECONDS} seconds.",
                self.name()
            ));
        }
        match self {
            Self::DeadlineSeconds => config.deadline_tick = (value / STEP_SECONDS) as u64,
            Self::ReloadSeconds => config.reload_ticks = (value / STEP_SECONDS) as u64,
            Self::SensorProcessingSeconds => {
                config.sensor_processing_ticks = (value / STEP_SECONDS) as u64
            }
            Self::ReactionSeconds => config.policy.reaction_seconds = value,
            Self::SafetyRadiusM => config.policy.safety_radius = value,
            Self::HoldSeconds => config.policy.hold_seconds = value,
            Self::RecoverySeconds => config.policy.recovery_seconds = value,
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Edit {
    pub tick: u64,
    pub parameter: Parameter,
    pub value: f64,
    pub during_play: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    Standard,
    Custom,
    Debug,
}

impl RunKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Custom => "custom",
            Self::Debug => "debug",
        }
    }
}
