use atomic_float::AtomicF32;
use bevy::prelude::*;
use bevy_audio_v2::{AudioPlugin, NodeId, UpdateAudioGraphExt};
use firewheel::graph::AudioGraph;
use firewheel::node::{AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcInfo};
use firewheel::BlockFrames;
use std::error::Error;
use std::f32::consts::TAU;
use std::sync::atomic::Ordering;
use std::sync::Arc;

struct BeepPlugin;

impl Plugin for BeepPlugin {
    fn build(&self, app: &mut App) {
        app.observe(on_add_beep).observe(on_remove_beep);
    }
}

fn on_add_beep(trigger: Trigger<OnAdd, Beep>, mut commands: Commands) {
    commands.entity(trigger.entity()).update_audio_graph(
        |mut entity: EntityWorldMut, audio_graph: &mut AudioGraph<(), 512>| {
            let beep = entity.get::<Beep>().unwrap();
            let node: Box<dyn AudioNode<_, 512>> = Box::new(BeepNode(Arc::new(BeepNodeImpl {
                amplitude: AtomicF32::new(beep.amplitude),
                frequency: AtomicF32::new(beep.frequency),
            })));
            let node = audio_graph.add_node(0, 1, node);
            audio_graph
                .connect(node, 0, audio_graph.graph_out_node(), 0, false)
                .unwrap();
            audio_graph
                .connect(node, 0, audio_graph.graph_out_node(), 1, false)
                .unwrap();
            entity.insert(NodeId(node));
        },
    );
}

fn on_remove_beep(trigger: Trigger<OnRemove, Beep>, mut commands: Commands) {
    commands.entity(trigger.entity()).update_audio_graph(
        |mut entity, audio_graph| {
            let NodeId(node_id) = *entity.get::<NodeId>().unwrap();
            audio_graph.remove_node(node_id).unwrap();
            entity.remove::<Beep>();
        },
    )
}

#[derive(Debug)]
struct BeepNodeImpl {
    amplitude: AtomicF32,
    frequency: AtomicF32,
}

#[derive(Debug, Clone, Deref)]
struct BeepNode(Arc<BeepNodeImpl>);

impl<C, const MBF: usize> AudioNode<C, MBF> for BeepNode {
    fn debug_name(&self) -> &'static str {
        "beep"
    }

    fn info(&self) -> AudioNodeInfo {
        AudioNodeInfo {
            num_min_supported_inputs: 0,
            num_max_supported_inputs: 0,
            num_min_supported_outputs: 1,
            num_max_supported_outputs: 1,
        }
    }

    fn activate(
        &mut self,
        sample_rate: u32,
        num_inputs: usize,
        num_outputs: usize,
    ) -> Result<Box<dyn AudioNodeProcessor<C, MBF>>, Box<dyn Error>> {
        Ok(Box::new(BeepNodeProcessor {
            params: self.clone(),
            discretization_factor: TAU / sample_rate as f32,
            phase: 0.,
        }))
    }
}

struct BeepNodeProcessor {
    params: BeepNode,
    discretization_factor: f32,
    phase: f32,
}

impl<C, const MBF: usize> AudioNodeProcessor<C, MBF> for BeepNodeProcessor {
    fn process(
        &mut self,
        frames: BlockFrames<MBF>,
        inputs: &[&[f32; MBF]],
        outputs: &mut [&mut [f32; MBF]],
        proc_info: ProcInfo<C>,
    ) {
        let step = self.params.frequency.load(Ordering::Relaxed) * self.discretization_factor;
        let amplitude = self.params.amplitude.load(Ordering::Relaxed);
        for i in 0..frames.get() {
            outputs[0][i] = self.phase.sin() * amplitude;
            self.phase += step;
        }
    }
}

#[derive(Debug, Copy, Clone, Component)]
struct Beep {
    amplitude: f32,
    frequency: f32,
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AudioPlugin, BeepPlugin))
        .add_systems(Update, toggle_beep)
        .run();
}

fn toggle_beep(
    mut current_entity: Local<Option<Entity>>,
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        if let Some(entity) = *current_entity {
            commands.entity(entity).despawn();
        } else {
            let entity = commands
                .spawn(Beep {
                    frequency: 440.,
                    amplitude: 0.125,
                })
                .id();
            current_entity.replace(entity);
        }
    }
}
