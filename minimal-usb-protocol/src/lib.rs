#![cfg_attr(not(test), no_std)]

use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(target_os = "none", derive(defmt::Format))]
pub struct MinimalConfig {
    samples: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(target_os = "none", derive(defmt::Format))]
pub enum SensorToHost {
    /// Identification to the host with hash of device ID, JEDEC ID, and EEPROM ID.
    /// Reaction to a [`HostToSensor::WhoAreYou`]
    Id { name: [u8; 24], version: u16 },

    /// Reaction to a ping ([`HostToSensor::Ping`]).
    Pong,

    /// Current Configuration.
    Config(MinimalConfig),

    /// Configuration was persisted OK.
    ConfigOk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(target_os = "none", derive(defmt::Format))]
pub enum HostToSensor {
    /// Trigger a sample process.
    Trigger,

    /// Request an identification ([`SensorToHost::Id`]).
    WhoAreYou,

    /// Request a pong ([`SensorToHost::Pong`]).
    Ping,

    /// Set Configuration.
    SetConfig(MinimalConfig),

    /// Get Configuration.
    GetConfig,

    /// Reset, for example to enter config mode.
    Reset,
}
