use core::render::material::MaterialRendererFactory;

pub struct PhongMaterialRendererFactory {}

impl MaterialRendererFactory for PhongMaterialRendererFactory {
    fn setup(
        &self,
        materials: &[std::sync::Arc<core::material::Material>],
        gpu: &core::backends::wgpu_backend::WGPUResource,
        g: &mut core::graph::rdg::RenderGraphBuilder,
        setup_resource: &core::render::material::SetupResource,
    ) {
    }
}
