use bevy::{
  prelude::*,
  reflect::TypeUuid,
  render::{
    mesh::shape,
    pipeline::{PipelineDescriptor, RenderPipeline},
    render_graph::{base, AssetRenderResourcesNode, RenderGraph},
    renderer::{RenderResource, RenderResourceIterator, RenderResources},
    shader::ShaderStages,
  },
};
use bevy_asset_ron::*;

use std::sync::{Arc, RwLock};
use serde::Deserialize;
use indexmap::{IndexMap, IndexSet};

lazy_static::lazy_static! {
  static ref NAME_TO_INDEX: Arc<RwLock<IndexSet<String>>> = {
    Arc::new(RwLock::new(IndexSet::new()))
  };
}

fn name_to_idx(name: &str) -> usize {
  let (idx, _) = NAME_TO_INDEX.write().unwrap().insert_full(name.into());
  idx
}


#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum CapsuleUvProfile {
  Aspect,
  Uniform,
  Fixed,
}

impl From<CapsuleUvProfile> for shape::CapsuleUvProfile {
  fn from(uv_profile: CapsuleUvProfile) -> Self {
    use shape::CapsuleUvProfile::*;
    match uv_profile {
      CapsuleUvProfile::Aspect => Aspect,
      CapsuleUvProfile::Uniform => Uniform,
      CapsuleUvProfile::Fixed => Fixed,
    }
  }
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub enum ObjectShape {
  Box(f32, f32, f32),
  Capsule {
    radius: f32,
    rings: usize,
    depth: f32,
    latitudes: usize,
    longitudes: usize,
    uv_profile: CapsuleUvProfile,
  },
  Cube(f32),
  Icosphere {
    radius: f32,
    subdivisions: usize,
  },
  Plane(f32),
  Quad {
    size: Vec2,
    flip: bool,
  },
  Torus {
    radius: f32,
    ring_radius: f32,
    subdivisions_segments: usize,
    subdivisions_sides: usize,
  }
}

impl ObjectShape {
  fn mesh(&self) -> Mesh {
    match *self {
      ObjectShape::Box(x, y, z) =>
        Mesh::from(shape::Box::new(x, y, z)),
      ObjectShape::Capsule { radius, rings, depth, latitudes, longitudes, uv_profile } =>
        Mesh::from(shape::Capsule {
          radius, rings, depth, latitudes, longitudes,
          uv_profile: uv_profile.into(),
        }),
      ObjectShape::Cube(size) =>
        Mesh::from(shape::Cube::new(size)),
      ObjectShape::Icosphere { radius, subdivisions } =>
        Mesh::from(shape::Icosphere {
          radius, subdivisions
        }),
      ObjectShape::Plane(size) =>
        Mesh::from(shape::Plane { size }),
      ObjectShape::Quad { size, flip } =>
        Mesh::from(shape::Quad {
          size, flip
        }),
      ObjectShape::Torus { radius, ring_radius, subdivisions_segments, subdivisions_sides } =>
        Mesh::from(shape::Torus {
          radius, ring_radius, subdivisions_segments, subdivisions_sides
        }),
    }
  }
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct MaterialPipeline {
  vertex: String,
  fragment: Option<String>,
}

impl MaterialPipeline {
  fn loading(&self, asset_server: &AssetServer) -> LoadingPipeline {
    let vertex = asset_server.load::<Shader, _>(self.vertex.as_str());
    let fragment = self.fragment.as_ref()
      .map(|frag| asset_server.load::<Shader, _>(frag.as_str()));
    LoadingPipeline {
      vertex,
      fragment
    }
  }
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub enum MaterialResourceType {
  Color,
  Texture,
}

#[derive(Deserialize, TypeUuid, Clone, Debug, PartialEq)]
#[uuid = "1f4cf560-7085-11ec-a4a8-5f7c5c7eb330"]
pub struct MaterialType {
  name: String,
  pipeline: MaterialPipeline,
  resource_types: IndexMap<String, MaterialResourceType>,
}

impl MaterialType {
  fn loading(&self, asset_server: &AssetServer) -> LoadingPipeline {
    self.pipeline.loading(asset_server)
  }
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub enum MaterialResource {
  Color(Color),
  Texture(String),
}

#[derive(Deserialize, TypeUuid, Clone, Debug, PartialEq)]
#[uuid = "37a754ea-7088-11ec-946d-0bd1d4b13458"]
pub struct MaterialSettings {
  material_type: String,
  resources: IndexMap<String, MaterialResource>,
}

impl MaterialSettings {
  fn loading(&self, asset_server: &AssetServer) -> LoadingMaterialType {
    let material_type = asset_server.load(self.material_type.as_str());
    LoadingMaterialType {
      material_type
    }
  }
}

#[derive(Deserialize, TypeUuid, Clone, Debug, PartialEq)]
#[uuid = "4593f266-7001-11ec-8b43-975982e15bbe"]
pub struct ObjectAsset {
  shape: ObjectShape,
  translation: [f32; 3],
  material: MaterialSettings,
}

#[derive(TypeUuid, Default)]
#[uuid = "1b6d822c-7001-11ec-b8af-bbb1f7c4e78e"]
pub struct CustomMaterial {
  resources: IndexMap<usize, (String, Box<dyn RenderResource + Send + Sync>)>,
}

impl CustomMaterial {
  pub fn new() -> Self {
    Self {
      resources: IndexMap::new()
    }
  }

  pub fn insert<T: 'static + RenderResource + Send + Sync>(&mut self, name: &str, resource: T) {
    let idx = name_to_idx(name);
    self.resources.insert(idx, (name.into(), Box::new(resource)));
  }
}

impl RenderResources for CustomMaterial {
  fn render_resources_len(&self) -> usize {
    self.resources.len()
  }

  fn get_render_resource(&self, index: usize) -> Option<&dyn RenderResource> {
    self.resources.get(&index)
      .map(|(_, res)| res.as_ref() as &dyn RenderResource)
  }

  fn get_render_resource_name(&self, index: usize) -> Option<&str> {
    self.resources.get(&index)
      .map(|(key, _)| key.as_str())
  }

  fn iter(&self) -> RenderResourceIterator {
    RenderResourceIterator::new(self)
  }
}

struct UpdateObject;
struct LoadedPipeline {
  render_pipeline: RenderPipeline,
}

struct LoadingMaterialType {
  material_type: Handle<MaterialType>,
}

struct LoadingPipeline {
  vertex: Handle<Shader>,
  fragment: Option<Handle<Shader>>,
}

fn loading_material_type(
  query: Query<(Entity, &LoadingMaterialType)>,
  material_types: Res<Assets<MaterialType>>,
  mut commands: Commands,
  asset_server: Res<AssetServer>,
) {
  for (entity, loading) in query.iter() {
    // Check if the material type definition is loaded.
    let material_type = match material_types.get(&loading.material_type) {
      Some(material_type) => material_type,
      None => {
        // Still loading.
        continue;
      }
    };

    eprintln!("MaterialType loaded: {:#?}", material_type);
    commands.entity(entity)
      .remove::<LoadingMaterialType>()
      .insert(material_type.loading(&asset_server))
      .insert(loading.material_type.clone());
  }
}

fn loading_pipeline(
  query: Query<(Entity, &LoadingPipeline, &ObjectAsset)>,
  shaders: Res<Assets<Shader>>,
  mut commands: Commands,
  mut pipelines: ResMut<Assets<PipelineDescriptor>>,
) {
  for (entity, loading, obj) in query.iter() {
    // Check if the shaders have loaded.
    if let Some(frag_handle) = &loading.fragment {
      if shaders.get(frag_handle).is_none() {
        // Fragment is not loaded yet.
        continue;
      }
    }
    if shaders.get(&loading.vertex).is_none() {
      // Vertex is not loaded yet.
      continue;
    }

    eprintln!("Shaders are loaded: {:#?}", obj);
    // Create a new shader pipeline with shaders loaded from the asset directory
    let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
      vertex: loading.vertex.clone(),
      fragment: loading.fragment.clone(),
    }));

    commands.entity(entity)
      .remove::<LoadingPipeline>()
      .insert(LoadedPipeline {
        render_pipeline: RenderPipeline::new(pipeline_handle),
      });
  }
}

fn spawn_object(
  query: Query<(Entity, &LoadedPipeline, &ObjectAsset)>,
  mut commands: Commands,
  asset_server: Res<AssetServer>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<CustomMaterial>>,
) {
  for (entity, loaded, obj) in query.iter() {
    eprintln!("Shader pipeline is loaded: {:#?}", obj);

    // Create a new material
    let mut material = CustomMaterial::new();
    for (key, res) in &obj.material.resources {
      match res {
        MaterialResource::Color(color) =>
          material.insert(key, *color),
        MaterialResource::Texture(texture) => {
          let texture: Handle<Texture> = asset_server.load(texture.as_str());
          material.insert(key, texture);
        },
      }
    }
    let material = materials.add(material);

    commands.entity(entity)
      .remove::<LoadedPipeline>()
      .insert_bundle(MeshBundle {
        mesh: meshes.add(obj.shape.mesh()),
        render_pipelines: RenderPipelines::from_pipelines(vec![
          loaded.render_pipeline.clone()
        ]),
        transform: Transform::from_translation(obj.translation.into()),
        ..Default::default()
      })
      .insert(material);
  }
}

fn update_objects(
  mut query: Query<(Entity, &mut ObjectAsset, &Handle<ObjectAsset>, &Handle<Mesh>, &mut Transform, &Handle<CustomMaterial>), With<UpdateObject>>,
  objects: Res<Assets<ObjectAsset>>,
  mut materials: ResMut<Assets<CustomMaterial>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut commands: Commands,
  asset_server: Res<AssetServer>,
) {
  for (entity, mut obj, handle, mesh, mut transform, material) in query.iter_mut() {
    if let Some(new_obj) = objects.get(handle) {
      // Moved.
      if new_obj.translation != obj.translation {
        info!("Move object: {:?}", new_obj.translation);
        transform.translation = new_obj.translation.into();
      }

      // Material changed.
      if new_obj.material != obj.material {
        info!("Update material: {:?}", new_obj.material);
        if let Some(material) = materials.get_mut(material) {
          for (key, res) in &new_obj.material.resources {
            match res {
              MaterialResource::Color(color) =>
                material.insert(key, *color),
              MaterialResource::Texture(texture) => {
                let texture: Handle<Texture> = asset_server.load(texture.as_str());
                material.insert(key, texture);
              },
            }
          }
        }
      }

      // Shape changed.
      if new_obj.shape != obj.shape {
        info!("Update shape: {:?}", new_obj.shape);
        if let Some(mesh) = meshes.get_mut(mesh) {
          *mesh = new_obj.shape.mesh();
        }
      }

      *obj = new_obj.clone();
      commands.entity(entity)
        .remove::<UpdateObject>();
    }
  }
}

fn watch_objects(
  mut query: Query<(Entity, &Handle<ObjectAsset>)>,
  objects: Res<Assets<ObjectAsset>>,
  mut events: EventReader<AssetEvent<ObjectAsset>>,
  mut commands: Commands,
  asset_server: Res<AssetServer>,
) {
  for event in events.iter() {
    let (is_create, handle) = match event {
      AssetEvent::Created { handle } => (true, Some(handle)),
      AssetEvent::Modified { handle } => (false, Some(handle)),
      _ => (false, None),
    };
    // Make sure that the object is loaded
    let handle_obj = handle.and_then(|handle| {
      objects.get(handle).map(|obj| (handle, obj))
    });
    if let Some((handle, obj)) = handle_obj {
      for (entity, obj_handle) in query.iter_mut() {
        if obj_handle != handle { continue; }
        if is_create {
          info!("Loaded object: {:#?}", obj);
          // Need to make sure the shaders are loaded before creating the pipeline.
          commands.entity(entity)
            .insert(obj.clone())
            .insert(obj.material.loading(&asset_server));
        } else {
          commands.entity(entity)
            .insert(UpdateObject);
        }
      }
    }
  }
}

fn setup(
  mut render_graph: ResMut<RenderGraph>,
) {
  // Add an AssetRenderResourcesNode to our Render Graph. This will bind CustomMaterial resources to
  // our shader
  render_graph.add_system_node(
    "custom_material",
    AssetRenderResourcesNode::<CustomMaterial>::new(true),
  );

  // Add a Render Graph edge connecting our new "custom_material" node to the main pass node. This
  // ensures "custom_material" runs before the main pass
  render_graph
    .add_node_edge("custom_material", base::node::MAIN_PASS)
    .unwrap();
}

/// CustomMaterialPlugin - For loading custom materials from files.
#[derive(Default, Clone, Debug)]
pub struct CustomMaterialPlugin;

impl Plugin for CustomMaterialPlugin {
  fn build(&self, app: &mut AppBuilder) {
    app
      // load objects from .obj files.
      .add_plugin(RonAssetPlugin::<ObjectAsset>::new(&["obj"]))
      // load material types from .material_type files.
      .add_plugin(RonAssetPlugin::<MaterialType>::new(&["material_type"]))
      // load materials from .material files.
      .add_plugin(RonAssetPlugin::<MaterialSettings>::new(&["material"]))
      .add_startup_system(setup.system())
      .add_system(loading_material_type.system())
      .add_system(loading_pipeline.system())
      .add_system(spawn_object.system())
      .add_system(watch_objects.system())
      .add_system(update_objects.system())
      .add_asset::<CustomMaterial>()
      ;

  }
}
