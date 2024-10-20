use crate::{AudioGraph, UpdateAudioGraphExt};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::world::EntityWorldMut;
use bevy_utils::EntityHashMap;
use firewheel::graph::NodeID;
use std::marker::PhantomData;

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
        app.init_resource::<NodeIds<N>>().observe(on_add_node::<N>).observe(on_remove_node::<N>);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioGraph, AudioPlugin};
    use bevy_app::App;
    use firewheel::node::AudioNode;

    #[derive(Default, Component)]
    struct TestNodeComponent {
        created: bool,
    }

    impl NodeComponent for TestNodeComponent {
        fn create_node(mut entity: EntityWorldMut, audio_graph: &mut AudioGraph) -> NodeID {
            entity.get_mut::<Self>().unwrap().created = true;
            let node: Box<dyn AudioNode<_, 512>> = Box::new(firewheel::basic_nodes::DummyAudioNode);
            audio_graph.add_node(0, 1, node)
        }
    }

    fn app() -> (App, Entity) {
        let mut app = App::default();
        app.add_plugins((AudioPlugin, NodePlugin::<TestNodeComponent>::default()));
        app.finish();
        app.cleanup();
        let entity = app.world_mut().spawn(TestNodeComponent::default()).id();
        (app, entity)
    }

    #[test]
    fn test_on_add_node() {
        let (mut app, entity) = app();

        app.update();

        let node_ids = app.world().get_resource::<NodeIds<TestNodeComponent>>().unwrap();
        let comp = app.world().entity(entity).get::<TestNodeComponent>().unwrap();
        assert!(comp.created);
        assert!(node_ids.data.contains_key(&entity));
    }

    #[test]
    fn test_on_remove_node() {
        let (mut app, entity) = app();
        app.update();

        app.world_mut().entity_mut(entity).remove::<TestNodeComponent>();
        app.update();

        let node_ids = app.world().get_resource::<NodeIds<TestNodeComponent>>().unwrap();
        assert!(!node_ids.data.contains_key(&entity));
    }
}