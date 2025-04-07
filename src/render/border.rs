use smithay::{
    backend::renderer::{
        Color32F,
        element::Kind,
        gles::{
            GlesPixelProgram, GlesRenderer, Uniform, UniformName, UniformType,
            element::PixelShaderElement,
        },
    },
    utils::{Logical, Rectangle},
};

const BORDER_SHADER: &str = include_str!("shaders/border.frag");

// Define a struct that holds a pixel shader. This struct will be stored in the data of the
// EGL rendering context.
pub struct BorderShader(pub GlesPixelProgram);

pub fn compile_shaders(renderer: &mut GlesRenderer) {
    // Compile GLSL file into pixel shader.
    let border_shader = renderer
        .compile_custom_pixel_shader(
            BORDER_SHADER,
            &[
                UniformName::new("u_resolution", UniformType::_2f),
                UniformName::new("border_color", UniformType::_3f),
                UniformName::new("border_thickness", UniformType::_1f),
            ],
        )
        .unwrap();

    // Save pixel shader in EGL rendering context.
    renderer
        .egl_context()
        .user_data()
        .insert_if_missing(|| BorderShader(border_shader));
}

impl BorderShader {
    pub fn element(
        renderer: &GlesRenderer,
        geometry: Rectangle<i32, Logical>,
        alpha: f32,
    ) -> PixelShaderElement {
        let program = renderer
            .egl_context()
            .user_data()
            .get::<BorderShader>()
            .unwrap()
            .0
            .clone();

        // TODO: use config
        let border_color: Color32F = Color32F::from([1.0, 0.0, 0.0, 1.0]);
        let border_thickness = 5.0;

        let point = geometry.size.to_point();

        PixelShaderElement::new(
            program,
            geometry,
            None,
            alpha,
            vec![
                Uniform::new("u_resolution", (point.x as f32, point.y as f32)),
                Uniform::new(
                    "border_color",
                    (border_color.r(), border_color.g(), border_color.b()),
                ),
                Uniform::new("border_thickness", border_thickness),
            ],
            Kind::Unspecified,
        )
    }
}

