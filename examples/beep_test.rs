use atomic_float::AtomicF32;
use bevy::prelude::Val::Px;
use bevy::prelude::*;
use bevy_audio_v2::{AudioPlugin, UpdateAudioGraphExt};
use bevy_utils::EntityHashMap;
use firewheel::graph::NodeID;
use firewheel::node::{AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcInfo};
use firewheel::BlockFrames;
use std::error::Error;
use std::f32::consts::TAU;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use bevy::log;

struct BeepPlugin;

#[derive(Debug, Default, Resource)]
struct Beeps {
    entities: EntityHashMap<Entity, NodeID>,
}

impl Plugin for BeepPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Beeps>()
            .observe(on_add_beep)
            .observe(on_remove_beep);
    }
}

fn on_add_beep(trigger: Trigger<OnAdd, Beep>, mut commands: Commands) {
    commands
        .entity(trigger.entity())
        .update_audio_graph(|world, entity, audio_graph| {
            let beep = world.entity(entity).get::<Beep>().unwrap();
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
            world.resource_mut::<Beeps>().entities.insert(entity, node);
        });
}

fn on_remove_beep(trigger: Trigger<OnRemove, Beep>, mut commands: Commands) {
    commands
        .entity(trigger.entity())
        .update_audio_graph(|world, entity, audio_graph| {
            let node = world
                .resource_mut::<Beeps>()
                .entities
                .remove(&entity)
                .unwrap();
            audio_graph.remove_node(node).unwrap();
        })
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

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AudioPlugin, BeepPlugin))
        .add_systems(Startup, setup_ui)
        .add_systems(Update, toggle_beep)
        .observe(ui_handle_added)
        .observe(ui_handle_despawned)
        .run();
}

fn toggle_beep(
    mut current_entity: Local<Option<Entity>>,
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        if let Some(entity) = current_entity.take() {
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
        });
}

fn ui_handle_added(
    trigger: Trigger<OnAdd, Beep>,
    mut commands: Commands,
    mut q_ui: Query<&mut Text, With<ActiveEntityMarker>>,
) {
    let mut text = q_ui.single_mut();
    text.sections[0].value = String::from("Yes");
    text.sections[0].style.color = COLOR_YES;
    commands
        .entity(trigger.entity());
}

fn ui_handle_despawned(
    _: Trigger<OnRemove, Beep>,
    mut q_ui: Query<&mut Text, With<ActiveEntityMarker>>,
    q: Query<(), With<Beep>>,
) {
    let mut text = q_ui.single_mut();
    let count = q.into_iter().count();
    log::info!("[on despawned] query count: {count}");

    if count > 1 { // Entity not despawned/component not removed yet
        text.sections[0].value = String::from("Yes");
        text.sections[0].style.color = COLOR_YES;
    } else {
        text.sections[0].value = String::from("No");
        text.sections[0].style.color = COLOR_NO;
    }
}
