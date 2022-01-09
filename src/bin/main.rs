use bevy::{
  prelude::*,
  pbr::*,
  reflect::TypeUuid,
  render::{
    camera::PerspectiveProjection,
  },
};
use bevy_asset_ron::*;

use ::material_designer::*;

/// This example illustrates how to load shaders such that they can be
/// edited while the example is still running.
fn main() {
  App::build()
    .add_plugins(DefaultPlugins)
    .add_plugin(bevy_jpeg2k::Jpeg2KPlugin)
    // load objects from .obj files.
    .add_plugin(RonAssetPlugin::<CameraSettings>::new(&["camera"]))
    // CustomMaterial Plugin
    .add_plugin(CustomMaterialPlugin)
    .add_startup_system(setup.system())
    .add_system(watch_camera.system())
    .run();
}

#[derive(serde::Deserialize, TypeUuid)]
#[uuid = "b7f64775-6e72-4080-9ced-167607f1f0b2"]
struct CameraSettings {
  translation: [f32; 3],
  fov_degrees: f32,
}

fn watch_camera(
  mut query: Query<(&mut PerspectiveProjection, &mut Transform, &Handle<CameraSettings>)>,
  settings: Res<Assets<CameraSettings>>,
  mut events: EventReader<AssetEvent<CameraSettings>>
) {
  for event in events.iter() {
    match event {
      AssetEvent::Created { handle } | AssetEvent::Modified { handle } => {
        if let Some(s) = settings.get(handle) {
          for (mut perspective, mut transform, cam_handle) in query.iter_mut() {
            if cam_handle == handle {
              transform.translation = s.translation.into();
              transform.look_at(Vec3::ZERO, Vec3::Y);
              perspective.fov = s.fov_degrees * std::f32::consts::PI / 180.0;
            }
          }
        }
      }
      _ => {
        // ignore remove events.
      }
    }
  }
}

fn setup(
  mut commands: Commands,
  asset_server: Res<AssetServer>,
) {
  // Watch for changes.
  asset_server.watch_for_changes().unwrap();

  // Load the camera settings.
  let cam_settings: Handle<CameraSettings> = asset_server.load("settings.camera");

  // light
  commands.spawn_bundle(LightBundle {
    transform: Transform::from_xyz(4.0, 8.0, 4.0),
    ..Default::default()
  });
  // ambient light
  commands.insert_resource(AmbientLight {
      color: Color::WHITE,
      brightness: 0.40,
  });

  // Create camera.
  commands
    .spawn_bundle(PerspectiveCameraBundle {
      transform: Transform::from_xyz(3.0, 5.0, -8.0).looking_at(Vec3::ZERO, Vec3::Y),
      ..Default::default()
    })
    .insert(cam_settings);

  // Load and spawn objects.
  let objs = asset_server.load_folder("objects").unwrap();
  for obj in objs {
    commands.spawn().insert(obj.typed::<ObjectAsset>());
  }
}
