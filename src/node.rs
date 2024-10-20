use crate::{AudioGraph, UpdateAudioGraphExt};
use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use bevy_ecs::world::EntityWorldMut;
use bevy_utils::EntityHashMap;
use firewheel::graph::NodeID;
use std::marker::PhantomData;

#[derive(Event)]
pub struct OnChange;

#[allow(unused_variables)]
pub trait NodeComponent: Component {
    fn create_node(entity: EntityWorldMut, audio_graph: &mut AudioGraph) -> NodeID;
    fn remove_node(entity: EntityWorldMut, audio_graph: &mut AudioGraph, node_id: NodeID) {
        audio_graph.remove_node(node_id).unwrap();
    }
}

pub struct NodePlugin<N: NodeComponent>(PhantomData<fn() -> N>);

impl<N: NodeComponent> Default for NodePlugin<N> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[derive(Debug, Resource)]
struct NodeIds<N: 'static + NodeComponent> {
    data: EntityHashMap<Entity, NodeID>,
    __node: PhantomData<fn() -> N>,
}

impl<N: NodeComponent> Default for NodeIds<N> {
    fn default() -> Self {
        Self {
            data: EntityHashMap::default(),
            __node: PhantomData,
        }
    }
}

impl<N: NodeComponent + 'static> Plugin for NodePlugin<N> {
    fn build(&self, app: &mut App) {
        app.init_resource::<NodeIds<N>>().observe(on_add_node::<N>).observe(on_remove_node::<N>).add_systems(PostUpdate, detect_changes::<N>);
    }
}

fn on_add_node<N: NodeComponent>(trigger: Trigger<OnAdd, N>, mut commands: Commands) {
    commands.entity(trigger.entity()).update_audio_graph(|world, entity, audio_graph| {
        let entity_mut = world.entity_mut(entity);
        let node_id = N::create_node(entity_mut, audio_graph);
        world.resource_mut::<NodeIds<N>>().data.insert(entity, node_id);
    });
}

fn on_remove_node<N: NodeComponent>(trigger: Trigger<OnRemove, N>, mut commands: Commands) {
    commands.entity(trigger.entity()).update_audio_graph(|world, entity, audio_graph| {
        let node_id = world.resource_mut::<NodeIds<N>>().data.remove(&entity).unwrap();
        let entity_mut = world.entity_mut(entity);
        N::remove_node(entity_mut, audio_graph, node_id);
    });
}

fn detect_changes<N: NodeComponent>(mut commands: Commands, q: Query<Entity, Changed<N>>) {
    for entity in &q {
        commands.trigger_targets(OnChange, entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioGraph;
    use bevy_app::App;
    use bevy_ecs::world::CommandQueue;
    use firewheel::node::AudioNode;

    #[derive(Component)]
    struct TestNodeComponent {
        created: bool,
        removed: bool,
    }

    impl NodeComponent for TestNodeComponent {
        fn create_node(mut entity: EntityWorldMut, audio_graph: &mut AudioGraph) -> NodeID {
            entity.get_mut::<Self>().unwrap().created = true;
            let node: Box<dyn AudioNode<_, 512>> = Box::new(firewheel::basic_nodes::DummyAudioNode);
            audio_graph.add_node(0, 1, node)
        }
    }

    #[test]
    fn test_on_add_node() {
        let mut app = App::default();
        app.world.spawn().insert(TestNodeComponent);
        app.init_resource::<NodeIds<TestNodeComponent>>();

        let mut command_queue = CommandQueue::default();
        let mut commands = Commands::new(&mut command_queue, &app.world);

        let entity = app.world.spawn().insert(TestNodeComponent).id();
        on_add_node(Trigger::new(entity), commands);

        command_queue.apply(&mut app.world);

        let node_ids = app.world.get_resource::<NodeIds<TestNodeComponent>>().unwrap();
        assert!(node_ids.data.contains_key(&entity));
    }

    #[test]
    fn test_on_remove_node() {
        let mut app = App::default();
        app.world.spawn().insert(TestNodeComponent);
        app.init_resource::<NodeIds<TestNodeComponent>>();

        let entity = app.world.spawn().insert(TestNodeComponent).id();

        let mut command_queue = CommandQueue::default();
        let mut commands = Commands::new(&mut command_queue, &app.world);
        on_add_node(Trigger::new(entity), commands.clone());
        command_queue.apply(&mut app.world);

        on_remove_node(Trigger::new(entity), commands);
        command_queue.apply(&mut app.world);

        let node_ids = app.world.get_resource::<NodeIds<TestNodeComponent>>().unwrap();
        assert!(!node_ids.data.contains_key(&entity));
    }

    #[test]
    fn test_detect_changes() {
        let mut app = App::default();
        app.world.spawn().insert(TestNodeComponent);
        app.init_resource::<NodeIds<TestNodeComponent>>();

        let entity = app.world.spawn().insert(TestNodeComponent).id();

        let mut command_queue = CommandQueue::default();
        let mut commands = Commands::new(&mut command_queue, &app.world);
        app.world.set_changed::<TestNodeComponent>(entity);

        detect_changes(commands, app.world.query::<Entity, Changed<TestNodeComponent>>());
        command_queue.apply(&mut app.world);

        // Here, you can add assertions to verify the behavior of `detect_changes`, 
        // such as whether the `OnChange` event was triggered.
        // Assert based on your actual application event testing mechanism.
    }
}