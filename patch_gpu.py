import re

with open("src/gpu/compute_node.rs", "r") as f:
    content = f.read()

# 1. Change setup_gpu_simulation signature
new_setup = """pub fn setup_gpu_simulation(
    commands: &mut Commands,
    render_dev: &RenderDevice,
    disk_params: &DiskParameters,
) {"""
content = re.sub(r'pub fn setup_gpu_simulation\([^)]+\)\s*\{', new_setup, content, count=1)

# Remove the old early return inside setup_gpu_simulation
content = re.sub(r'let \(Some\(render_dev\), Some\(_queue\)\) = \(render_device, render_queue\) else \{\n\s*return;\n\s*\};\n\s*let device = render_dev\.wgpu_device\(\);', 'let device = render_dev.wgpu_device();', content, count=1)

# 2. Change step_gpu_simulation signature
new_step = """pub fn step_gpu_simulation(
    mut commands: Commands,
    render_device: Option<Res<RenderDevice>>,
    render_queue: Option<Res<RenderQueue>>,
    gpu_engine: Option<Res<GpuSimulationEngine>>,
    particle_swarm: Option<ResMut<ParticleSwarmData>>,
    config: Res<SimulationConfig>,
    time_warp: Res<TimeWarp>,
    player_state: Res<PlayerInteractionState>,
    disk_params: Res<DiskParameters>,
    star_query: Query<
        (
            &SimPosition,
            &Mass,
            &Radius,
            &Temperature,
            &Luminosity,
            &IgnitionState,
        ),
        With<CentralStar>,
    >,
    massive_query: Query<(&SimPosition, &Mass, &CelestialBody), Without<CentralStar>>,
) {"""
content = re.sub(r'pub fn step_gpu_simulation\([^)]+\)\s*\{', new_step, content, count=1)

# 3. Change step_gpu_simulation logic
old_logic = """    let (Some(render_dev), Some(queue), Some(engine)) = (render_device, render_queue, gpu_engine)
    else {
        return;
    };
    let device = render_dev.wgpu_device();

    if !engine.is_ready {
        return;
    }"""
new_logic = """    let (Some(render_dev), Some(queue_res)) = (&render_device, &render_queue) else {
        return;
    };
    
    if gpu_engine.is_none() {
        setup_gpu_simulation(&mut commands, render_dev, &disk_params);
        return;
    }
    
    let engine = gpu_engine.unwrap();
    let queue = queue_res;
    let device = render_dev.wgpu_device();

    if !engine.is_ready {
        return;
    }"""
content = content.replace(old_logic, new_logic)

with open("src/gpu/compute_node.rs", "w") as f:
    f.write(content)
