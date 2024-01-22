use std::f32::consts::PI;

use macroquad::{prelude::*, ui::root_ui};
use miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};

const VIRTUAL_WIDTH: f32 = 1280.0;
const VIRTUAL_HEIGHT: f32 = 720.0;

// source: https://github.com/code-disaster/gdx-mellow-demo
// adapted from `mod shader` from quad_gl.rs in miniquad
const VERTEX_SHADER: &'static str = "#version 100
precision lowp float;

attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

uniform vec4 u_textureSizes;
uniform vec4 u_sampleProperties;

varying vec2 v_texCoords;

void main() {
    color = color0 / 255.0;

    vec2 uvSize = u_textureSizes.xy;
    float upscale = u_textureSizes.z;

    v_texCoords.x = texcoord.x + (u_sampleProperties.z / upscale) / uvSize.x;
    v_texCoords.y = (texcoord.y + (u_sampleProperties.w / upscale) / uvSize.y);

    gl_Position = Projection * Model * vec4(position, 1);

    uv = texcoord;
}
";

const FRAGMENT_SHADER: &'static str = "#version 100
precision lowp float;

varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

varying vec2 v_texCoords;
uniform vec4 u_textureSizes;
uniform vec4 u_sampleProperties;

void main() {
    vec2 uv = v_texCoords;
    vec2 uvSize = u_textureSizes.xy;
    float upscale = u_textureSizes.z;

    gl_FragColor = color * texture2D(Texture, uv);
}
";

#[macroquad::main("Pixel Perfect")]
async fn main() {
    // Setup 'render_target', used to hold the rendering result so we can resize it
    let mut render_targ = render_target(VIRTUAL_WIDTH as u32, VIRTUAL_HEIGHT as u32);
    render_targ.texture.set_filter(FilterMode::Nearest);

    let rustacean_tex = load_texture("examples/chicken.png").await.unwrap();
    rustacean_tex.set_filter(FilterMode::Nearest);

    // Setup camera for the virtual screen, that will render to 'render_target'
    let mut render_targ_cam =
        Camera2D::from_display_rect(Rect::new(0., 0., VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
    render_targ_cam.render_target = Some(render_targ.clone());

    let material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX_SHADER,
            fragment: FRAGMENT_SHADER,
        },
        MaterialParams {
            uniforms: vec![
                ("u_textureSizes".to_owned(), UniformType::Float4),
                ("u_sampleProperties".to_owned(), UniformType::Float4),
            ],
            pipeline_params: PipelineParams {
                depth_write: false,
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();

    let mut scale = 4.0;

    // Desired behavior:
    // Given scale factor, the screen width/height will not divide by it evenly always.
    // In that case, we need to find the gap.

    let mut last_res_and_scale = (Vec2::ZERO, scale);
    let mut camera_offset_ideal = vec2(0., 0.);
    let camera_speed = 0.1;

    let mut freelook = true;

    let dt = 1. / 60.;
    let mut camera_angle = 0.0;

    let mut use_shader = true;

    let mut timer = 0.;

    loop {
        timer += dt;

        if is_key_pressed(KeyCode::Equal) {
            scale *= 2.0;
            println!("Scale up is now: {}", scale)
        } else if is_key_pressed(KeyCode::Minus) && scale > 1.0 {
            scale /= 2.0;
            println!("Scale up is now: {}", scale)
        }

        let pressing =
            |key| is_key_pressed(key) || (is_key_down(KeyCode::LeftShift) && is_key_down(key));

        if pressing(KeyCode::W) {
            camera_offset_ideal.y -= camera_speed;
        } else if pressing(KeyCode::S) {
            camera_offset_ideal.y += camera_speed;
        }
        if pressing(KeyCode::A) {
            camera_offset_ideal.x -= camera_speed;
        } else if pressing(KeyCode::D) {
            camera_offset_ideal.x += camera_speed;
        }
        if is_key_pressed(KeyCode::R) {
            camera_offset_ideal = vec2(0., 0.);
        }
        if is_key_pressed(KeyCode::F) {
            freelook = !freelook;
            println!("Freelook is now: {}", freelook);
        }
        if is_key_pressed(KeyCode::P) {
            use_shader = !use_shader;
            println!("Use shader is now: {}", use_shader);
        }

        if !freelook {
            camera_offset_ideal =
                vec2(-10.0, -10.) + Vec2::from_angle(camera_angle).rotate(vec2(5., 5.));
            camera_angle += PI * dt;
        }

        let res = vec2(screen_width(), screen_height());
        let res_changed = res != last_res_and_scale.0 || last_res_and_scale.1 != scale;
        let leftover_pixels = vec2(res.x % scale, res.y % scale);
        // note extra pixel in each direction; the extra pixel will get upscaled and partially used
        // to fill in the "leftover pixels"
        let canvas_size =
            vec2(res.x - leftover_pixels.x, res.y - leftover_pixels.y) / scale + Vec2::ONE;

        if res_changed {
            render_targ = render_target(canvas_size.x as u32, canvas_size.y as u32);
            render_targ.texture.set_filter(FilterMode::Nearest);

            render_targ_cam =
                Camera2D::from_display_rect(Rect::new(0., 0., canvas_size.x, canvas_size.y));
            render_targ_cam.render_target = Some(render_targ.clone());
        }

        let camera_offset_pixel_aligned = camera_offset_ideal.floor();
        render_targ_cam.target = vec2(
            camera_offset_pixel_aligned.x + canvas_size.x / 2.,
            camera_offset_pixel_aligned.y + canvas_size.y / 2.,
        );
        root_ui().label(
            None,
            &format!(
                "Shader: {:?}, Camera target: {:?}",
                use_shader, render_targ_cam.target
            ),
        );

        // Mouse position in the virtual screen
        let virtual_mouse_pos_exact = Vec2 {
            x: (mouse_position().0 - (screen_width() - (canvas_size.x * scale)) * 0.5) / scale,
            y: (mouse_position().1 - (screen_height() - (canvas_size.y * scale)) * 0.5) / scale,
        };
        let virtual_mouse_pos = virtual_mouse_pos_exact.floor();

        // ------------------------------------------------------------------------
        // Begin drawing the virtual screen to 'render_target'
        // ------------------------------------------------------------------------
        set_camera(&render_targ_cam);

        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));

        draw_text("Hello Pixel Perfect", 20.0, 20.0, 16.0, DARKGRAY);
        draw_circle(65.0, 50., 20.0, GREEN);
        draw_circle(65.0 * 2.0, 65., 35.0, BLUE);

        for x in (0..10).step_by(2) {
            for y in (0..10).step_by(2) {
                draw_rectangle(
                    x as f32,
                    y as f32,
                    1.,
                    1.,
                    Color::new(x as f32 / 10.0, y as f32 / 10.0, (x + y) as f32 / 20.0, 1.0),
                );
            }
        }

        draw_circle(virtual_mouse_pos.x, virtual_mouse_pos.y, 5.0, BLACK);

        draw_circle(15.0 + (10. * timer.cos()).round(), 40., 5.0, ORANGE);

        draw_texture(
            &rustacean_tex,
            10.0 + (10. * timer.cos()).round(),
            60. + (10. * timer.sin()).round(),
            WHITE,
        );

        {
            //crosshair
            draw_line(
                virtual_mouse_pos.x.round(),
                virtual_mouse_pos.y.round() - 20.0,
                virtual_mouse_pos.x.round(),
                virtual_mouse_pos.y.round() + 20.0,
                1.0,
                RED,
            );

            draw_line(
                virtual_mouse_pos.x.round() - 20.0,
                virtual_mouse_pos.y.round(),
                virtual_mouse_pos.x.round() + 20.0,
                virtual_mouse_pos.y.round(),
                1.0,
                RED,
            );
            draw_circle(virtual_mouse_pos.x, virtual_mouse_pos.y, 5.0, BLACK);
        }

        // ------------------------------------------------------------------------
        // Begin drawing the window screen
        // ------------------------------------------------------------------------
        set_default_camera();

        clear_background(GRAY); // Will be the letterbox color

        root_ui().label(
            Some(vec2(0., screen_height() - 48.)),
            &format!("FPS={:?}", get_fps()),
        );
        root_ui().label(
            Some(vec2(0., screen_height() - 32.)),
            &format!(
                "Res={:?}, scale={:?}, image size={:?}, leftover={:?}",
                res, scale, canvas_size, leftover_pixels
            ),
        );
        root_ui().label(
            Some(vec2(0., screen_height() - 16.)),
            &format!(
                "camera_offset_ideal={:?}, camera_offset_pixel_aligned={:?}",
                camera_offset_ideal, camera_offset_pixel_aligned
            ),
        );

        // Draw 'render_target' to window screen, properly scaled
        if use_shader {
            let diff = ((camera_offset_ideal - camera_offset_pixel_aligned) * scale)
                .as_ivec2()
                .as_vec2();

            material.set_uniform("u_textureSizes", [canvas_size.x, canvas_size.y, scale, 0.0]);
            material.set_uniform("u_sampleProperties", [0.0, 0.0, diff.x, 1.0 - diff.y]); // 1-diff.y because texture gets v-flipped
            gl_use_material(&material);
        }
        draw_texture_ex(
            &render_targ.texture,
            0.,
            0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(canvas_size.x * scale, canvas_size.y * scale)),
                flip_y: true, // Must flip y otherwise 'render_target' will be upside down
                ..Default::default()
            },
        );

        {
            let v = vec2(10.0 + (10. * timer.cos()), 80. + (10. * timer.sin()));
            let v = render_targ_cam.world_to_screen(v);
            println!("v: {:?}", v);
            let s = rustacean_tex.size() * scale;
            draw_texture_ex(
                &rustacean_tex,
                v.x,
                v.y,
                RED,
                DrawTextureParams {
                    dest_size: Some(s),
                    ..Default::default()
                },
            );
        }

        gl_use_default_material();

        next_frame().await;

        last_res_and_scale = (res, scale);
    }
}
