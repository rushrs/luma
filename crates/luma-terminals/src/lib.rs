// SPDX-License-Identifier: MIT
mod ghostty;
mod tmux;

pub use ghostty::Ghostty;
pub use tmux::{Tmux, tmux_config_file, tmux_theme_file};
