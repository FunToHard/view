//! Development qualification only; these functions are not view's public API.
#![forbid(unsafe_code)]

/// A direct cosmic-text value must also be glyphon's re-exported type.
pub fn text_type_compatibility(fonts: cosmic_text::FontSystem) -> glyphon::FontSystem {
    fonts
}

/// Type-check the actual renderer/device integration without requiring a GPU.
pub fn text_renderer_compatibility(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (glyphon::Cache, glyphon::TextAtlas) {
    let cache = glyphon::Cache::new(device);
    let atlas = glyphon::TextAtlas::new(device, queue, &cache, wgpu::TextureFormat::Rgba8Unorm);
    (cache, atlas)
}

/// Type-check native accessibility's window/event and semantic update boundary.
pub fn accessibility_compatibility(
    adapter: &mut accesskit_winit::Adapter,
    window: &winit::window::Window,
    event: &winit::event::WindowEvent,
    update: accesskit::TreeUpdate,
) {
    adapter.process_event(window, event);
    adapter.update_if_active(|| update);
}

#[cfg(test)]
mod tests {
    use lyon::math::point;
    use lyon::path::Path;
    use lyon::tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
    };
    use unicode_segmentation::UnicodeSegmentation;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct Vertex {
        position: [f32; 2],
    }

    #[test]
    fn unicode_fixture_preserves_grapheme_boundaries() {
        let fixture = include_str!("../../../tests/fixtures/text/graphemes.txt").trim_end();
        let clusters: Vec<_> = fixture.graphemes(true).collect();
        assert_eq!(clusters, ["e\u{301}", "👩‍💻", "🇮🇳"]);
        assert_eq!(clusters.concat(), fixture);
    }

    #[test]
    fn tessellation_produces_expected_triangle_area_and_upload_layout() {
        let mut builder = Path::builder();
        builder.begin(point(0.0, 0.0));
        builder.line_to(point(2.0, 0.0));
        builder.line_to(point(0.0, 2.0));
        builder.close();
        let mut mesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();
        FillTessellator::new()
            .tessellate_path(
                &builder.build(),
                &FillOptions::default(),
                &mut BuffersBuilder::new(&mut mesh, |vertex: FillVertex<'_>| Vertex {
                    position: vertex.position().to_array(),
                }),
            )
            .expect("triangle tessellation");
        let area: f32 = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|indices| {
                let [a, b, c] = [indices[0], indices[1], indices[2]]
                    .map(|i| mesh.vertices[usize::from(i)].position);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum();
        assert!((area - 2.0).abs() < 1e-6);
        let bytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
        assert_eq!(bytes.len(), mesh.vertices.len() * 8);
    }
}
