use crate::{
    core_3d::graph::{Core3d, Node3d},
    fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    prelude::Camera3d,
    prepass::{DepthPrepass, MotionVectorPrepass, ViewPrepassTextures},
};
use bevy_app::{App, Plugin};
use bevy_asset::{load_internal_asset, Handle};
use bevy_core::FrameCount;
use bevy_ecs::{
    prelude::{Bundle, Component, Entity},
    query::{QueryItem, With},
    schedule::IntoSystemConfigs,
    system::{Commands, Query, Res, ResMut, Resource},
    world::{FromWorld, World},
};
use bevy_math::vec2;
use bevy_reflect::Reflect;
use bevy_render::{
    camera::{ExtractedCamera, MipBias, TemporalJitter},
    prelude::{Camera, Projection},
    render_graph::{NodeRunError, RenderGraphApp, RenderGraphContext, ViewNode, ViewNodeRunner},
    render_resource::{
        binding_types::{sampler, texture_2d, texture_depth_2d},
        BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedRenderPipelineId,
        ColorTargetState, ColorWrites, Extent3d, FilterMode, FragmentState, MultisampleState,
        Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
        RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, Shader,
        ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines, TextureDescriptor,
        TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    },
    renderer::{RenderContext, RenderDevice},
    texture::{BevyDefault, CachedTexture, TextureCache},
    view::{ExtractedView, Msaa, ViewTarget},
    ExtractSchedule, MainWorld, Render, RenderApp, RenderSet,
};

const TAA_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(656865235226276);

/// Plugin for temporal anti-aliasing. Disables multisample anti-aliasing (MSAA).
///
/// See [`TemporalAntiAliasSettings`] for more details.
pub struct TemporalAntiAliasPlugin;

impl Plugin for TemporalAntiAliasPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, TAA_SHADER_HANDLE, "taa.wgsl", Shader::from_wgsl);

        app.insert_resource(Msaa::Off)
            .register_type::<TemporalAntiAliasSettings>();

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<SpecializedRenderPipelines<TaaPipeline>>()
            .add_systems(ExtractSchedule, extract_taa_settings)
            .add_systems(
                Render,
                (
                    prepare_taa_jitter_and_mip_bias.in_set(RenderSet::ManageViews),
                    prepare_taa_pipelines.in_set(RenderSet::Prepare),
                    prepare_taa_history_textures.in_set(RenderSet::PrepareResources),
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<TemporalAntiAliasNode>>(Core3d, Node3d::Taa)
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::EndMainPass,
                    Node3d::Taa,
                    Node3d::MotionBlur,
                    Node3d::Bloom,
                    Node3d::Tonemapping,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<TaaPipeline>();
    }
}

/// Bundle to apply temporal anti-aliasing.
#[derive(Bundle, Default)]
pub struct TemporalAntiAliasBundle {
    pub settings: TemporalAntiAliasSettings,
    pub jitter: TemporalJitter,
    pub depth_prepass: DepthPrepass,
    pub motion_vector_prepass: MotionVectorPrepass,
}

/// Component to apply temporal anti-aliasing to a 3D perspective camera.
///
/// Temporal anti-aliasing (TAA) is a form of image smoothing/filtering, like
/// multisample anti-aliasing (MSAA), or fast approximate anti-aliasing (FXAA).
/// TAA works by blending (averaging) each frame with the past few frames.
///
/// # Tradeoffs
///
/// Pros:
/// * Filters more types of aliasing than MSAA, such as textures and singular bright pixels (specular aliasing)
/// * Cost scales with screen/view resolution, unlike MSAA which scales with number of triangles
/// * Greatly increases the quality of stochastic rendering techniques such as SSAO, certain shadow map sampling methods, etc
///
/// Cons:
/// * Chance of "ghosting" - ghostly trails left behind moving objects
/// * Thin geometry, lighting detail, or texture lines may flicker noisily or disappear
///
/// Because TAA blends past frames with the current frame, when the frames differ too much
/// (such as with fast moving objects or camera cuts), ghosting artifacts may occur.
///
/// Artifacts tend to be reduced at higher framerates and rendering resolution.
///
/// # Usage Notes
///
/// Requires that you add [`TemporalAntiAliasPlugin`] to your app,
/// and add the [`DepthPrepass`], [`MotionVectorPrepass`], and [`TemporalJitter`]
/// components to your camera.
///
/// [Currently](https://github.com/bevyengine/bevy/issues/8423) cannot be used with [`bevy_render::camera::OrthographicProjection`].
///
/// Currently does not support skinned meshes and morph targets.
/// There will probably be ghosting artifacts if used with them.
/// Does not work well with alpha-blended meshes as it requires depth writing to determine motion.
///
/// It is very important that correct motion vectors are written for everything on screen.
/// Failure to do so will lead to ghosting artifacts. For instance, if particle effects
/// are added using a third party library, the library must either:
/// 1. Write particle motion vectors to the motion vectors prepass texture
/// 2. Render particles after TAA
///
/// If no [`MipBias`] component is attached to the camera, TAA will add a MipBias(-1.0) component.
#[derive(Component, Reflect, Clone)]
pub struct TemporalAntiAliasSettings {
    /// Set to true to delete the saved temporal history (past frames).
    ///
    /// Useful for preventing ghosting when the history is no longer
    /// representative of the current frame, such as in sudden camera cuts.
    ///
    /// After setting this to true, it will automatically be toggled
    /// back to false at the end of the frame.
    pub reset: bool,
}

impl Default for TemporalAntiAliasSettings {
    fn default() -> Self {
        Self { reset: true }
    }
}

/// Render [`bevy_render::render_graph::Node`] used by temporal anti-aliasing.
#[derive(Default)]
pub struct TemporalAntiAliasNode;

impl ViewNode for TemporalAntiAliasNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ViewTarget,
        &'static TemporalAntiAliasHistoryTextures,
        &'static ViewPrepassTextures,
        &'static TemporalAntiAliasPipelineId,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (camera, view_target, taa_history_textures, prepass_textures, taa_pipeline_id): QueryItem<
            Self::ViewQuery,
        >,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let (Some(pipelines), Some(pipeline_cache)) = (
            world.get_resource::<TaaPipeline>(),
            world.get_resource::<PipelineCache>(),
        ) else {
            return Ok(());
        };
        let (Some(taa_pipeline), Some(prepass_motion_vectors_texture), Some(prepass_depth_texture)) = (
            pipeline_cache.get_render_pipeline(taa_pipeline_id.0),
            &prepass_textures.motion_vectors,
            &prepass_textures.depth,
        ) else {
            return Ok(());
        };
        let view_target = view_target.post_process_write();

        let taa_bind_group = render_context.render_device().create_bind_group(
            "taa_bind_group",
            &pipelines.taa_bind_group_layout,
            &BindGroupEntries::sequential((
                view_target.source,
                &taa_history_textures.read.default_view,
                &prepass_motion_vectors_texture.texture.default_view,
                &prepass_depth_texture.texture.default_view,
                &pipelines.nearest_sampler,
                &pipelines.linear_sampler,
            )),
        );

        {
            let mut taa_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("taa_pass"),
                color_attachments: &[
                    Some(RenderPassColorAttachment {
                        view: view_target.destination,
                        resolve_target: None,
                        ops: Operations::default(),
                    }),
                    Some(RenderPassColorAttachment {
                        view: &taa_history_textures.write.default_view,
                        resolve_target: None,
                        ops: Operations::default(),
                    }),
                ],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            taa_pass.set_render_pipeline(taa_pipeline);
            taa_pass.set_bind_group(0, &taa_bind_group, &[]);
            if let Some(viewport) = camera.viewport.as_ref() {
                taa_pass.set_camera_viewport(viewport);
            }
            taa_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}

#[derive(Resource)]
struct TaaPipeline {
    taa_bind_group_layout: BindGroupLayout,
    nearest_sampler: Sampler,
    linear_sampler: Sampler,
}

impl FromWorld for TaaPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let nearest_sampler = render_device.create_sampler(&SamplerDescriptor {
            label: Some("taa_nearest_sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..SamplerDescriptor::default()
        });
        let linear_sampler = render_device.create_sampler(&SamplerDescriptor {
            label: Some("taa_linear_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..SamplerDescriptor::default()
        });

        let taa_bind_group_layout = render_device.create_bind_group_layout(
            "taa_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // View target (read)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // TAA History (read)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // Motion Vectors
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // Depth
                    texture_depth_2d(),
                    // Nearest sampler
                    sampler(SamplerBindingType::NonFiltering),
                    // Linear sampler
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );

        TaaPipeline {
            taa_bind_group_layout,
            nearest_sampler,
            linear_sampler,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct TaaPipelineKey {
    hdr: bool,
    reset: bool,
}

impl SpecializedRenderPipeline for TaaPipeline {
    type Key = TaaPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = vec![];

        let format = if key.hdr {
            shader_defs.push("TONEMAP".into());
            ViewTarget::TEXTURE_FORMAT_HDR
        } else {
            TextureFormat::bevy_default()
        };

        if key.reset {
            shader_defs.push("RESET".into());
        }

        RenderPipelineDescriptor {
            label: Some("taa_pipeline".into()),
            layout: vec![self.taa_bind_group_layout.clone()],
            vertex: fullscreen_shader_vertex_state(),
            fragment: Some(FragmentState {
                shader: TAA_SHADER_HANDLE,
                shader_defs,
                entry_point: "taa".into(),
                targets: vec![
                    Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    }),
                    Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    }),
                ],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: Vec::new(),
        }
    }
}

fn extract_taa_settings(mut commands: Commands, mut main_world: ResMut<MainWorld>) {
    let mut cameras_3d = main_world
        .query_filtered::<(Entity, &Camera, &Projection, &mut TemporalAntiAliasSettings), (
            With<Camera3d>,
            With<TemporalJitter>,
            With<DepthPrepass>,
            With<MotionVectorPrepass>,
        )>();

    for (entity, camera, camera_projection, mut taa_settings) in
        cameras_3d.iter_mut(&mut main_world)
    {
        let has_perspective_projection = matches!(camera_projection, Projection::Perspective(_));
        if camera.is_active && has_perspective_projection {
            commands.get_or_spawn(entity).insert(taa_settings.clone());
            taa_settings.reset = false;
        }
    }
}

fn prepare_taa_jitter_and_mip_bias(
    frame_count: Res<FrameCount>,
    mut query: Query<
        (Entity, &mut TemporalJitter, Option<&MipBias>),
        With<TemporalAntiAliasSettings>,
    >,
    mut commands: Commands,
) {
    // Halton sequence (2, 3) - 0.5, skipping i = 0
    let halton_sequence = [
        vec2(0.0, -0.16666666),
        vec2(-0.25, 0.16666669),
        vec2(0.25, -0.3888889),
        vec2(-0.375, -0.055555552),
        vec2(0.125, 0.2777778),
        vec2(-0.125, -0.2777778),
        vec2(0.375, 0.055555582),
        vec2(-0.4375, 0.3888889),
    ];

    let offset = halton_sequence[frame_count.0 as usize % halton_sequence.len()];

    for (entity, mut jitter, mip_bias) in &mut query {
        jitter.offset = offset;

        if mip_bias.is_none() {
            commands.entity(entity).insert(MipBias(-1.0));
        }
    }
}

#[derive(Component)]
pub struct TemporalAntiAliasHistoryTextures {
    write: CachedTexture,
    read: CachedTexture,
}

fn prepare_taa_history_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    frame_count: Res<FrameCount>,
    views: Query<(Entity, &ExtractedCamera, &ExtractedView), With<TemporalAntiAliasSettings>>,
) {
    for (entity, camera, view) in &views {
        if let Some(physical_viewport_size) = camera.physical_viewport_size {
            let mut texture_descriptor = TextureDescriptor {
                label: None,
                size: Extent3d {
                    depth_or_array_layers: 1,
                    width: physical_viewport_size.x,
                    height: physical_viewport_size.y,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: if view.hdr {
                    ViewTarget::TEXTURE_FORMAT_HDR
                } else {
                    TextureFormat::bevy_default()
                },
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            };

            texture_descriptor.label = Some("taa_history_1_texture");
            let history_1_texture = texture_cache.get(&render_device, texture_descriptor.clone());

            texture_descriptor.label = Some("taa_history_2_texture");
            let history_2_texture = texture_cache.get(&render_device, texture_descriptor);

            let textures = if frame_count.0 % 2 == 0 {
                TemporalAntiAliasHistoryTextures {
                    write: history_1_texture,
                    read: history_2_texture,
                }
            } else {
                TemporalAntiAliasHistoryTextures {
                    write: history_2_texture,
                    read: history_1_texture,
                }
            };

            commands.entity(entity).insert(textures);
        }
    }
}

#[derive(Component)]
pub struct TemporalAntiAliasPipelineId(CachedRenderPipelineId);

fn prepare_taa_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<TaaPipeline>>,
    pipeline: Res<TaaPipeline>,
    views: Query<(Entity, &ExtractedView, &TemporalAntiAliasSettings)>,
) {
    for (entity, view, taa_settings) in &views {
        let mut pipeline_key = TaaPipelineKey {
            hdr: view.hdr,
            reset: taa_settings.reset,
        };
        let pipeline_id = pipelines.specialize(&pipeline_cache, &pipeline, pipeline_key.clone());

        // Prepare non-reset pipeline anyways - it will be necessary next frame
        if pipeline_key.reset {
            pipeline_key.reset = false;
            pipelines.specialize(&pipeline_cache, &pipeline, pipeline_key);
        }

        commands
            .entity(entity)
            .insert(TemporalAntiAliasPipelineId(pipeline_id));
    }
}
