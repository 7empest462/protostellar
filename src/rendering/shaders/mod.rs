//! Protostellar Embedded WESL Shader Libraries.
//!
//! Registers reusable WESL astrophysics, noise, scattering, blackbody, hypsometry,
//! and planetary procedural modules with explicit RAII handle retention in ECS resources,
//! avoiding `mem::forget` on `Handle<Shader>` drop semantics.

use bevy::app::{App, Plugin};
use bevy::asset::{embedded_asset, load_embedded_asset, Handle};
use bevy::ecs::resource::Resource;
use bevy_shader::Shader;

/// Resource permanently holding strong handles to embedded astrophysics WESL shader libraries.
#[derive(Resource, Default, Debug)]
pub struct ProtostellarEmbeddedShaders(pub Vec<Handle<Shader>>);

pub struct ProtostellarShadersPlugin;

impl Plugin for ProtostellarShadersPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "noise.wesl");
        let h_noise: Handle<Shader> = load_embedded_asset!(app, "noise.wesl");

        embedded_asset!(app, "scattering.wesl");
        let h_scattering: Handle<Shader> = load_embedded_asset!(app, "scattering.wesl");

        embedded_asset!(app, "blackbody.wesl");
        let h_blackbody: Handle<Shader> = load_embedded_asset!(app, "blackbody.wesl");

        embedded_asset!(app, "hypsometry.wesl");
        let h_hypsometry: Handle<Shader> = load_embedded_asset!(app, "hypsometry.wesl");

        embedded_asset!(app, "gas_giants.wesl");
        let h_gas_giants: Handle<Shader> = load_embedded_asset!(app, "gas_giants.wesl");

        embedded_asset!(app, "cutaway.wesl");
        let h_cutaway: Handle<Shader> = load_embedded_asset!(app, "cutaway.wesl");

        app.insert_resource(ProtostellarEmbeddedShaders(vec![
            h_noise,
            h_scattering,
            h_blackbody,
            h_hypsometry,
            h_gas_giants,
            h_cutaway,
        ]));
    }
}
