use wgpu;
fn main() {
    let _x = wgpu::PollType::Wait { submission_index: None, timeout: None };
}
