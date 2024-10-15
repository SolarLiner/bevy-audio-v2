use bevy::prelude::*;
use bevy_audio_v2::AudioPlugin;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AudioPlugin))
        .run();
}