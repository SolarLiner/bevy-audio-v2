use bevy_app::prelude::*;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{EntityCommand, EntityCommands};
use bevy_ecs::world::Command;
use bevy_log as log;
use bevy_log::prelude::*;
use firewheel::graph::{AudioGraph as FirewheelGraph, NodeID};
use firewheel::{ActiveFwCpalCtx, InactiveFwCpalCtx, UpdateStatus};
use log::error;

pub mod node;

const DEFAULT_MAX_BLOCK_FRAMES: usize = 512;

#[derive(Deref, DerefMut)]
pub struct AudioEngineBuilder(InactiveFwCpalCtx<BevyContext, DEFAULT_MAX_BLOCK_FRAMES>);

impl Default for AudioEngineBuilder {
    fn default() -> Self {
        Self(InactiveFwCpalCtx::new(Default::default()))
    }
}

#[derive(Deref, DerefMut)]
pub struct AudioEngine(ActiveFwCpalCtx<BevyContext, DEFAULT_MAX_BLOCK_FRAMES>);

#[derive(Debug)]
#[non_exhaustive]
pub struct BevyContext;

pub type AudioGraph = FirewheelGraph<BevyContext, 512>;

#[derive(Debug, Clone, Resource)]
pub struct InputDevice(pub String);

#[derive(Debug, Clone, Resource)]
pub struct OutputDevice(pub String);

#[derive(Debug, Copy, Clone, Deref, DerefMut, Component)]
pub struct NodeId(pub NodeID);

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<AudioEngineBuilder>();
        app.add_systems(Last, update_audio_engine).add_systems(
            PostUpdate,
            update_output_device.run_if(resource_exists_and_changed::<OutputDevice>),
        );
    }

    fn finish(&self, app: &mut App) {
        let AudioEngineBuilder(cx) = app
            .world_mut()
            .remove_non_send_resource::<AudioEngineBuilder>()
            .unwrap();
        // let input_device = app.world().get_resource::<InputDevice>().map(|s| &s.0);
        let output_device = app.world().get_resource::<OutputDevice>().map(|s| &s.0);
        let cx = cx
            .activate(output_device, true, BevyContext)
            .expect("Cannot start audio engine");
        app.insert_non_send_resource(AudioEngine(cx));
    }
}

fn update_output_device(world: &mut World) {
    let AudioEngine(cx) = world
        .remove_non_send_resource()
        .expect("Audio engine incorrectly set up");
    let OutputDevice(out_device) = world.resource();
    info!("Changing output device to {out_device:?}");

    let (cx, context) = cx.deactivate();
    let cx = cx
        .activate(Some(out_device), true, context.unwrap())
        .expect("Couldn't restart audio engine");
    world.insert_non_send_resource(AudioEngine(cx));
}

// Exclusive system because the audio engine requires moving itself out and back in
fn update_audio_engine(world: &mut World) {
    let Some(AudioEngine(cx)) = world
        .remove_non_send_resource() else {
        error!("Error getting audio engine resource");
        return;
    };
    if !world.resource::<Events<AppExit>>().is_empty() {
        cx.deactivate();
        info!("Shutting down audio engine");
        return;
    }

    match cx.update() {
        UpdateStatus::Ok { cx, graph_error } => {
            if let Some(error) = graph_error {
                log::error!("Audio graph error: {error}");
            }
            world.insert_non_send_resource(AudioEngine(cx));
        }
        UpdateStatus::Deactivated { error_msg, .. } => {
            info!("Audio engine deactivated");
            if let Some(error) = error_msg {
                log::error!("Audio engine error: {error}");
            }
        }
    }
}

fn apply_audio_graph_command(
    world: &mut World,
    apply: impl FnOnce(&mut World, &mut AudioGraph),
) {
    let Some(mut cx) = world
        .remove_non_send_resource::<AudioEngine>()
    else {
        error!("Audio Engine incorrectly set up");
        return;
    };
    let audio_graph = cx.graph_mut();
    apply(world, audio_graph);
    world.insert_non_send_resource(cx);
}

pub struct UpdateAudioGraphCommand<F>(F);

impl<F: 'static + Send + FnOnce(&mut World, &mut AudioGraph)> Command
for UpdateAudioGraphCommand<F>
{
    fn apply(self, world: &mut World) {
        apply_audio_graph_command(world, self.0);
    }
}

impl<F: 'static + Send + FnOnce(&mut World, Entity, &mut AudioGraph)> EntityCommand
for UpdateAudioGraphCommand<F>
{
    fn apply(self, id: Entity, world: &mut World) {
        apply_audio_graph_command(world, |world, audio_graph| {
            self.0(world, id, audio_graph);
        });
    }
}

pub trait UpdateAudioGraphExt<F> {
    fn update_audio_graph(&mut self, apply: F);
}

impl<'w, 's, F: 'static + Send + FnOnce(&mut World, &mut AudioGraph)>
UpdateAudioGraphExt<F> for Commands<'w, 's>
{
    fn update_audio_graph(&mut self, apply: F) {
        self.add(UpdateAudioGraphCommand(apply));
    }
}

impl<'a, F: 'static + Send + FnOnce(&mut World, Entity, &mut AudioGraph)>
UpdateAudioGraphExt<F> for EntityCommands<'a>
{
    fn update_audio_graph(&mut self, apply: F) {
        self.add(UpdateAudioGraphCommand(apply));
    }
}
