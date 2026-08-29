use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
pub fn print_if_render_device(rd: Option<Res<RenderDevice>>) {
    if rd.is_some() {
        println!("RenderDevice is available!");
    } else {
        println!("RenderDevice is NOT available!");
    }
}
