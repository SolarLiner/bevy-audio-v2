//! Demonstrates how to create a simple audio node, in this case generating a sine wave at a specific
//! frequency and amplitude.
//!
//! At a low level, there are 3 different types to manage:
//! - The bevy component: this is the source of truth. Ideally, all changes made to this component should
//!   be reflected in the audio node processor so that the sounds are also changed acordingly.
//! - The audio node: This type is responsible for creating the audio node processor from the component.
//! - The audio node processor: This type does the audio processing. It is running entirely separate from
//!   Bevy, so any changes need to be synchronized.
//!
//! In this example, we use shared atomics as a means of communicating the parameters between Bevy and the audio engine.
//! There are different solutions available, this is the simplest one to set up.
use atomic_float::AtomicF32;
use bevy::prelude::Val::Px;
use bevy::prelude::*;
use bevy_audio_v2::node::{NodeComponent, NodePlugin};
use bevy_audio_v2::{AudioGraph, AudioPlugin};
use firewheel::graph::NodeID;
use firewheel::node::{AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcInfo};
use firewheel::BlockFrames;
use std::error::Error;
use std::f32::consts::TAU;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Debug)]
struct BeepNodeImpl {
    amplitude: AtomicF32,
    frequency: AtomicF32,
}

/// Audio node type.
#[derive(Debug, Clone, Deref, Component)]
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
        _num_inputs: usize,
        _num_outputs: usize,
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
        _inputs: &[&[f32; MBF]],
        outputs: &mut [&mut [f32; MBF]],
        _proc_info: ProcInfo<C>,
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

impl NodeComponent for Beep {
    fn create_node(mut entity: EntityWorldMut, audio_graph: &mut AudioGraph) -> NodeID {
        let this = entity.get::<Beep>().unwrap();
        let node = BeepNode(Arc::new(BeepNodeImpl {
            amplitude: AtomicF32::new(this.amplitude),
            frequency: AtomicF32::new(this.frequency),
        }));
        entity.insert(node.clone());
        let node: Box<dyn AudioNode<_, 512>> = Box::new(node.clone());
        let node = audio_graph.add_node(0, 1, node);
        audio_graph
            .connect(node, 0, audio_graph.graph_out_node(), 0, false)
            .unwrap();
        audio_graph
            .connect(node, 0, audio_graph.graph_out_node(), 1, false)
            .unwrap();
        node
    }
}

fn on_change_beep(q: Query<(&Beep, &BeepNode), Changed<Beep>>) {
    for (beep, node) in &q {
        info!("Beep changed: amplitude = {}, frequency = {}", beep.amplitude, beep.frequency);
        node.amplitude.store(beep.amplitude, Ordering::Relaxed);
        node.frequency.store(beep.frequency, Ordering::Relaxed);
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AudioPlugin, NodePlugin::<Beep>::default()))
        .add_systems(Startup, (setup_beep, setup_ui))
        .add_systems(Update, toggle_beep)
        .add_systems(PostUpdate, (on_change_beep, handle_ui_changes.run_if(|q: Query<(), Changed<Beep>>| !q.is_empty())))
        .run();
}

fn setup_beep(mut commands: Commands) {
    commands.spawn((Beep { amplitude: 0., frequency: 440. }, ActiveEntityMarker));
}

fn toggle_beep(
    mut q: Query<&mut Beep, With<ActiveEntityMarker>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        let mut beep = q.single_mut();
        beep.amplitude = if beep.amplitude > f32::EPSILON { 0. } else { 1. };
    }
}

#[derive(Component)]
struct ActiveEntityMarker;

const COLOR_NO: Color = Color::srgb(0.9, 0.3, 0.1);
const COLOR_YES: Color = Color::srgb(0.2, 0.5, 1.0);

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Px(5.),
                left: Px(5.),
                display: Display::Flex,
                align_items: AlignItems::Start,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Entity is active: ",
                TextStyle {
                    font_size: 24.,
                    ..default()
                },
            ));
            parent.spawn((
                TextBundle::from_section(
                    "No",
                    TextStyle {
                        font_size: 24.,
                        color: COLOR_NO,
                        ..default()
                    },
                ),
                ActiveEntityMarker,
            ));
            parent.spawn(TextBundle::from_section(" (Press Space to toggle)", TextStyle { font_size: 24., ..default() }));
        });
}

fn handle_ui_changes(q: Query<&Beep, With<ActiveEntityMarker>>, mut q_ui: Query<&mut Text, With<ActiveEntityMarker>>) {
    let mut text = q_ui.single_mut();
    let beep = q.single();
    if beep.amplitude > f32::EPSILON {
        text.sections[0].value = String::from("Yes");
        text.sections[0].style.color = COLOR_YES;
    } else {
        text.sections[0].value = String::from("No");
        text.sections[0].style.color = COLOR_NO;
    }
}
