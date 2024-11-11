use bevy_ecs::{query::QueryItem, system::lifetimeless::Read, world::World};
use bevy_render::{
    extract_component::DynamicUniformIndex,
    render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
    render_resource::{
        ComputePassDescriptor, LoadOp, Operations, PipelineCache, RenderPassColorAttachment,
        RenderPassDescriptor, StoreOp,
    },
    renderer::RenderContext,
    view::{ViewTarget, ViewUniformOffset},
};

use crate::ViewLightsUniformOffset;

use super::{
    resources::{AtmosphereBindGroups, AtmospherePipelines, AtmosphereTextures},
    Atmosphere, AtmosphereSettings,
};

#[derive(PartialEq, Eq, Debug, Copy, Clone, Hash, RenderLabel)]
pub enum AtmosphereNode {
    RenderLuts,
    RenderSky,
}

#[derive(Default)]
pub(super) struct AtmosphereLutsNode {}

impl ViewNode for AtmosphereLutsNode {
    type ViewQuery = (
        Read<AtmosphereTextures>,
        Read<AtmosphereSettings>,
        Read<AtmosphereBindGroups>,
        Read<DynamicUniformIndex<Atmosphere>>,
        Read<DynamicUniformIndex<AtmosphereSettings>>,
        Read<ViewUniformOffset>,
        Read<ViewLightsUniformOffset>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (
            textures,
            settings,
            bind_groups,
            atmosphere_uniforms_offset,
            settings_uniforms_offset,
            view_uniforms_offset,
            lights_uniforms_offset,
        ): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipelines = world.resource::<AtmospherePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let (
            Some(transmittance_lut_pipeline),
            Some(multiscattering_lut_pipeline),
            Some(sky_view_lut_pipeline),
            Some(aerial_view_lut_pipeline),
        ) = (
            pipeline_cache.get_render_pipeline(pipelines.transmittance_lut),
            pipeline_cache.get_compute_pipeline(pipelines.multiscattering_lut),
            pipeline_cache.get_render_pipeline(pipelines.sky_view_lut),
            pipeline_cache.get_compute_pipeline(pipelines.aerial_view_lut),
        )
        else {
            //TODO: warning
            return Ok(());
        };

        let commands = render_context.command_encoder();

        commands.push_debug_group("atmosphere_luts");

        {
            let mut transmittance_lut_pass = commands.begin_render_pass(&RenderPassDescriptor {
                label: Some("transmittance_lut_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &textures.transmittance_lut.default_view,
                    resolve_target: None,
                    ops: Operations::default(),
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            transmittance_lut_pass.set_pipeline(transmittance_lut_pipeline);
            transmittance_lut_pass.set_bind_group(
                0,
                &bind_groups.transmittance_lut,
                &[
                    atmosphere_uniforms_offset.index(),
                    settings_uniforms_offset.index(),
                ],
            );
            transmittance_lut_pass.draw(0..3, 0..1);
        }

        //todo: use fragment shader here? maybe shared memory would be nice though
        {
            let mut multiscattering_lut_pass =
                commands.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("multiscatttering_lut_pass"),
                    timestamp_writes: None,
                });
            multiscattering_lut_pass.set_pipeline(multiscattering_lut_pipeline);
            multiscattering_lut_pass.set_bind_group(
                0,
                &bind_groups.multiscattering_lut,
                &[
                    atmosphere_uniforms_offset.index(),
                    settings_uniforms_offset.index(),
                ],
            );

            const MULTISCATTERING_WORKGROUP_SIZE: u32 = 16;
            let workgroups_x = settings
                .multiscattering_lut_size
                .x
                .div_ceil(MULTISCATTERING_WORKGROUP_SIZE);
            let workgroups_y = settings
                .multiscattering_lut_size
                .y
                .div_ceil(MULTISCATTERING_WORKGROUP_SIZE);

            multiscattering_lut_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        {
            let mut sky_view_lut_pass = commands.begin_render_pass(&RenderPassDescriptor {
                label: Some("sky_view_lut_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &textures.sky_view_lut.default_view,
                    resolve_target: None,
                    ops: Operations::default(),
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            sky_view_lut_pass.set_pipeline(sky_view_lut_pipeline);
            sky_view_lut_pass.set_bind_group(
                0,
                &bind_groups.sky_view_lut,
                &[
                    atmosphere_uniforms_offset.index(),
                    settings_uniforms_offset.index(),
                    view_uniforms_offset.offset,
                    lights_uniforms_offset.offset,
                ],
            );
            sky_view_lut_pass.draw(0..3, 0..1);
        }

        {
            let mut aerial_view_lut_pass = commands.begin_compute_pass(&ComputePassDescriptor {
                label: Some("aerial_view_lut_pass"),
                timestamp_writes: None,
            });
            aerial_view_lut_pass.set_pipeline(aerial_view_lut_pipeline);
            aerial_view_lut_pass.set_bind_group(
                0,
                &bind_groups.aerial_view_lut,
                &[
                    atmosphere_uniforms_offset.index(),
                    settings_uniforms_offset.index(),
                    view_uniforms_offset.offset,
                    lights_uniforms_offset.offset,
                ],
            );

            const AERIAL_VIEW_WORKGROUP_SIZE: u32 = 16;
            let workgroups_x = settings
                .aerial_view_lut_size
                .x
                .div_ceil(AERIAL_VIEW_WORKGROUP_SIZE);
            let workgroups_y = settings
                .aerial_view_lut_size
                .y
                .div_ceil(AERIAL_VIEW_WORKGROUP_SIZE);

            aerial_view_lut_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        render_context.command_encoder().pop_debug_group();
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct RenderSkyNode;

impl ViewNode for RenderSkyNode {
    type ViewQuery = (
        Read<AtmosphereBindGroups>,
        Read<ViewTarget>,
        Read<DynamicUniformIndex<Atmosphere>>,
        Read<DynamicUniformIndex<AtmosphereSettings>>,
        Read<ViewUniformOffset>,
        Read<ViewLightsUniformOffset>,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (
            atmosphere_bind_groups,
            view_target,
            atmosphere_uniforms_offset,
            settings_uniforms_offset,
            view_uniforms_offset,
            lights_uniforms_offset,
        ): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let atmosphere_pipelines = world.resource::<AtmospherePipelines>();
        let Some(render_sky_pipeline) =
            pipeline_cache.get_render_pipeline(atmosphere_pipelines.render_sky)
        else {
            return Ok(());
        }; //TODO: warning

        let mut render_sky_pass =
            render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("render_sky_pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: view_target.main_texture_view(),
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

        render_sky_pass.set_pipeline(render_sky_pipeline);
        render_sky_pass.set_bind_group(
            0,
            &atmosphere_bind_groups.render_sky,
            &[
                atmosphere_uniforms_offset.index(),
                settings_uniforms_offset.index(),
                view_uniforms_offset.offset,
                lights_uniforms_offset.offset,
            ],
        );
        render_sky_pass.draw(0..3, 0..1);

        Ok(())
    }
}
