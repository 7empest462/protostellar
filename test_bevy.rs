use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
fn main() {
    let device: RenderDevice = unimplemented!();
    device.poll(wgpu::MaintainBase::Wait);
}
