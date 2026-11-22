// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Akihito Kirisaki <kirisaki@klara.works>

pub mod ai;
pub mod maneuver;
pub mod map;
pub mod mission;
pub mod observation;
pub mod parameters;
pub mod physics;
pub mod planning;
pub mod presentation;
pub mod repl;
pub mod scenario;
pub mod session;
pub mod timing;
pub mod units;

#[cfg(target_arch = "wasm32")]
mod browser;
