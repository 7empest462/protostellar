//! Protostellar Embedded WESL Shader Libraries.
//!
//! Registers reusable WESL astrophysics, noise, scattering, blackbody, hypsometry,
//! and planetary procedural modules via `bevy_shader::load_shader_library!`.

use bevy::app::{App, Plugin};
use bevy_shader::load_shader_library;

pub struct ProtostellarShadersPlugin;

impl Plugin for ProtostellarShadersPlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "noise.wesl");
        load_shader_library!(app, "scattering.wesl");
        load_shader_library!(app, "blackbody.wesl");
        load_shader_library!(app, "hypsometry.wesl");
        load_shader_library!(app, "gas_giants.wesl");
        load_shader_library!(app, "cutaway.wesl");
    }
}
