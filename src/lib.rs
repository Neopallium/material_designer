use bevy::{
  prelude::*,
  reflect::TypeUuid,
  render::{
    mesh::{shape, Indices},
    pipeline::{PrimitiveTopology, PipelineDescriptor, RenderPipeline},
    render_graph::{base, AssetRenderResourcesNode, RenderGraph},
    renderer::{
      RenderResource, RenderResourceType,
      RenderResourceIterator, RenderResources,
    },
    shader::{asset_shader_defs_system, ShaderDefs, ShaderDefIterator, ShaderStages},
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

  static ref NULL_RESOURCE: NullResource = {
    NullResource{}
  };
}

fn name_to_idx(name: &str) -> usize {
  let (idx, _) = NAME_TO_INDEX.write().unwrap().insert_full(name.into());
  idx
}

fn names_length() -> usize {
  NAME_TO_INDEX.read().unwrap().len()
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
  Grid {
    size: u16,
    scale: f32,
  },
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
  fn generate_grid_mesh(size: u32, scale: f32) -> Mesh {
    let cap = (size as usize).pow(2);

    // Pre-calculate position & uv units.
    let uv_step = 1.0 / (size - 1) as f32;
    let pos_uv = (0..size).map(|n| {
      let pos = n as f32;
      let uv = uv_step * n as f32;
      (pos, uv)
    }).collect::<Vec<_>>();

    let mut positions = Vec::with_capacity(cap);
    let mut uvs = Vec::with_capacity(cap);

    for (pos_x, uv_x) in pos_uv.iter() {
      for (pos_z, uv_z) in pos_uv.iter() {
        let pos = [*pos_x * scale, 0.0, *pos_z * scale];
        let uv = [*uv_x, *uv_z];
        positions.push(pos);
        uvs.push(uv);
      }
    }

    let idx_cap = (cap - 2) * 2;
    let mut indices = Vec::with_capacity(idx_cap);
    for row_idx in 0..size-1 {
      let top_offset = row_idx * size;
      let btm_offset = top_offset + size;
      if row_idx > 0 {
        // Degenerate triangles.
        indices.push(top_offset);
        indices.push(btm_offset - 1);
      }
      for idx in (0..size).rev() {
        // Top vertices.
        indices.push(top_offset + idx);
        // Bottom vertices.
        indices.push(btm_offset + idx);
      }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleStrip);
    mesh.set_indices(Some(Indices::U32(indices)));
    mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

    mesh
  }

  fn mesh(&self) -> Mesh {
    match *self {
      ObjectShape::Box(x, y, z) =>
        Mesh::from(shape::Box::new(x, y, z)),
      ObjectShape::Capsule { radius, rings, depth, latitudes, longitudes, uv_profile } =>
        Mesh::from(shape::Capsule {
          radius, rings, depth, latitudes, longitudes,
          uv_profile: uv_profile.into(),
        }),
      ObjectShape::Grid { size, scale } =>
        Self::generate_grid_mesh(size as u32, scale),
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

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
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

#[derive(Deserialize, TypeUuid, Clone, Debug, Default, PartialEq)]
#[uuid = "1f4cf560-7085-11ec-a4a8-5f7c5c7eb330"]
pub struct MaterialType {
  name: String,
  pipeline: MaterialPipeline,
  resource_types: IndexMap<String, MaterialResourceType>,
  #[serde(skip)]
  shader_defs: IndexMap<String, String>,
}

impl MaterialType {
  fn init(&mut self) {
    for key in self.resource_types.keys() {
      self.shader_defs.insert(key.into(), key.to_uppercase());
    }
  }

  fn get_shader_def(&self, name: &str) -> Option<&str> {
    self.shader_defs.get(name).map(|s| s.as_str())
  }

  fn get_resource_idx(&self, name: &str) -> Option<usize> {
    self.resource_types.get_full(name.into()).map(|(idx, _, _)| idx)
  }

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
  material_type: MaterialType,
  resources: IndexMap<usize, (String, Box<dyn RenderResource + Send + Sync>)>,
}

impl CustomMaterial {
  pub fn new(material_type: &MaterialType) -> Self {
    Self {
      material_type: material_type.clone(),
      resources: IndexMap::new()
    }
  }

  pub fn insert<T: 'static + RenderResource + Send + Sync>(&mut self, name: &str, resource: T) -> bool {
    if let Some(_idx) = self.material_type.get_resource_idx(name) {
      let idx = name_to_idx(name);
      self.resources.insert(idx, (name.into(), Box::new(resource)));
      true
    } else {
      false
    }
  }
}

struct NullResource {}

impl RenderResource for NullResource {
  fn resource_type(&self) -> Option<RenderResourceType> {
    None
  }

  fn write_buffer_bytes(&self, _buffer: &mut [u8]) {}

  fn buffer_byte_len(&self) -> Option<usize> {
    None
  }

  fn texture(&self) -> Option<&Handle<Texture>> {
    None
  }
}

impl RenderResources for CustomMaterial {
  fn render_resources_len(&self) -> usize {
    // All `CustomMaterial` have to have the same number of resources.
    names_length()
  }

  fn get_render_resource(&self, index: usize) -> Option<&dyn RenderResource> {
    let res = self.resources.get(&index)
      .map(|(_, res)| res.as_ref() as &dyn RenderResource);
    res.or_else(|| {
      // We must always return a RenderResource.
      Some(&*NULL_RESOURCE as &dyn RenderResource)
    })
  }

  fn get_render_resource_name(&self, index: usize) -> Option<&str> {
    self.resources.get(&index)
      .map(|(key, _)| key.as_str())
  }

  fn iter(&self) -> RenderResourceIterator {
    RenderResourceIterator::new(self)
  }
}

// We use `ShaderDefs` to allow our custom material to have different
// resources.
impl ShaderDefs for CustomMaterial {
  fn shader_defs_len(&self) -> usize {
    // All `CustomMaterial` have to have the same number of resources.
    names_length()
  }

  fn get_shader_def(&self, index: usize) -> Option<&str> {
    let name = self.resources.get(&index)
      .and_then(|(name, _)| {
        self.material_type.get_shader_def(name)
      });
    name
  }

  fn iter_shader_defs(&self) -> ShaderDefIterator<'_> {
    ShaderDefIterator::new(self)
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
  mut material_types: ResMut<Assets<MaterialType>>,
  mut commands: Commands,
  asset_server: Res<AssetServer>,
) {
  for (entity, loading) in query.iter() {
    // Check if the material type definition is loaded.
    let material_type = match material_types.get_mut(&loading.material_type) {
      Some(material_type) => material_type,
      None => {
        // Still loading.
        continue;
      }
    };

    debug!("MaterialType loaded: {:#?}", material_type);
    material_type.init();
    commands.entity(entity)
      .remove::<LoadingMaterialType>()
      .insert(material_type.loading(&asset_server))
      .insert(material_type.clone());
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

    debug!("Shaders are loaded: {:#?}", obj);
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
  query: Query<(Entity, &LoadedPipeline, &ObjectAsset, &MaterialType)>,
  mut commands: Commands,
  asset_server: Res<AssetServer>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<CustomMaterial>>,
) {
  for (entity, loaded, obj, material_type) in query.iter() {
    debug!("Shader pipeline is loaded: {:#?}", material_type);

    info!("Spawn object: {:#?}", obj);
    // Create a new material
    let mut material = CustomMaterial::new(&material_type);
    for (key, res) in &obj.material.resources {
      let valid = match res {
        MaterialResource::Color(color) =>
          material.insert(key, *color),
        MaterialResource::Texture(texture) => {
          let texture: Handle<Texture> = asset_server.load(texture.as_str());
          material.insert(key, texture)
        },
      };
      if !valid {
        error!("Try to set an invalid resource field: {:?} => {:?}", key, res);
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
            let valid = match res {
              MaterialResource::Color(color) =>
                material.insert(key, *color),
              MaterialResource::Texture(texture) => {
                let texture: Handle<Texture> = asset_server.load(texture.as_str());
                material.insert(key, texture)
              },
            };
            if !valid {
              error!("Try to set an invalid resource field: {:?} => {:?}", key, res);
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
      .add_system_to_stage(
        CoreStage::PostUpdate,
        asset_shader_defs_system::<CustomMaterial>.system(),
      )
      .add_asset::<CustomMaterial>()
      ;

  }
}
