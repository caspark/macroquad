use std::f32::consts::PI;

use macroquad::{prelude::*, ui::root_ui};
use miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};

const VIRTUAL_WIDTH: f32 = 1280.0;
const VIRTUAL_HEIGHT: f32 = 720.0;

const VERTEX_SHADER: &'static str = "#version 130
precision lowp float;

attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

varying vec2 v_texCoords;

void main() {
    color = color0 / 255.0;

    gl_Position = Projection * Model * vec4(position, 1);

    uv = texcoord;
}
";

// source: https://www.shadertoy.com/view/ltfXWS
// via https://jorenjoestar.github.io/post/pixel_art_filtering/
const FRAGMENT_SHADER: &'static str = "#version 130
precision lowp float;

varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

// basically calculates the lengths of (a.x, b.x) and (a.y, b.y) at the same time
vec2 v2len(in vec2 a, in vec2 b) {
    return sqrt(a*a+b*b);
}

// samples from a linearly-interpolated texture to produce an appearance similar to
// nearest-neighbor interpolation, but with resolution-dependent antialiasing
//
// this function's interface is exactly the same as texture's, aside from the 'res'
// parameter, which represents the resolution of the texture 'tex'.
vec4 textureBlocky(in sampler2D tex, in vec2 uv, in vec2 res) {
    uv *= res; // enter texel coordinate space.


    vec2 seam = floor(uv+.5); // find the nearest seam between texels.

    // here's where the magic happens. scale up the distance to the seam so that all
    // interpolation happens in a one-pixel-wide space.
    uv = (uv-seam)/v2len(dFdx(uv),dFdy(uv))+seam;

    uv = clamp(uv, seam-.5, seam+.5); // clamp to the center of a texel.


    return texture(tex, uv/res, -1000.); // convert back to 0..1 coordinate space.
}

// simulates nearest-neighbor interpolation on a linearly-interpolated texture
//
// this function's interface is exactly the same as textureBlocky's.
vec4 textureUgly(in sampler2D tex, in vec2 uv, in vec2 res) {
    return textureLod(tex, (floor(uv*res)+.5)/res, 0.0);
}

void main() {
    ivec2 texsizeI = textureSize(Texture, 0);
    vec2 texsize = vec2(float(texsizeI.x), float(texsizeI.y));

    gl_FragColor = color * textureBlocky(Texture, uv, texsize);
}
";

#[macroquad::main("Pixel IM-perfect")]
async fn main() {
    // Setup 'render_target', used to hold the rendering result so we can resize it
    let mut render_targ = render_target(VIRTUAL_WIDTH as u32, VIRTUAL_HEIGHT as u32);
    render_targ.texture.set_filter(FilterMode::Nearest);

    let chicken_tex = load_texture("examples/chicken.png").await.unwrap();
    chicken_tex.set_filter(FilterMode::Linear);

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

    let mut last_res_and_scale = (Vec2::ZERO, scale);
    let mut camera_offset_ideal = vec2(0., 0.);
    let camera_speed = 1.1;

    let mut freelook = true;

    let dt = 1. / 60.;
    let mut camera_angle = 0.0;

    let mut use_shader = true;

    let mut timer = 0.;

    let mut keep_camera_pixel_aligned = true;

    loop {
        timer += dt;

        if is_key_pressed(KeyCode::Equal) {
            if scale >= 4.0 {
                scale *= 2.0;
            } else {
                scale += 1.0;
            }
            println!("Scale up is now: {}", scale)
        } else if is_key_pressed(KeyCode::Minus) && scale > 1.0 {
            if scale >= 4.0 {
                scale /= 2.0;
            } else {
                scale -= 1.0;
            }
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
            if use_shader {
                chicken_tex.set_filter(FilterMode::Linear);
            } else {
                chicken_tex.set_filter(FilterMode::Nearest);
            }
        }
        if is_key_pressed(KeyCode::V) {
            keep_camera_pixel_aligned = !keep_camera_pixel_aligned;
            println!("keep_camera_pixel_aligned is now: {}", keep_camera_pixel_aligned);
        }

        if !freelook {
            let displacement: Vec2 = Vec2::ONE * 50.0;
            camera_offset_ideal =
                -displacement + Vec2::from_angle(camera_angle).rotate(displacement);
            camera_angle += PI * dt;
        }

        let res = vec2(screen_width(), screen_height());
        let res_changed = res != last_res_and_scale.0 || last_res_and_scale.1 != scale;
        let leftover_pixels = vec2(res.x % scale, res.y % scale);
        // note extra pixel in each direction; the extra pixel will get upscaled and partially used
        // to fill in the "leftover pixels"
        let canvas_size = res;
        // vec2(res.x - leftover_pixels.x, res.y - leftover_pixels.y);

        if res_changed {
            render_targ = render_target(canvas_size.x as u32, canvas_size.y as u32);
            render_targ.texture.set_filter(FilterMode::Linear);

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
                "Shader: {:?}, keep camera pixel aligned: {:?}, Camera target: {:?}",
                use_shader, keep_camera_pixel_aligned, render_targ_cam.target
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
        if use_shader {
            gl_use_material(&material);
        } else {
            gl_use_default_material();
        }
        // set_camera(&render_targ_cam);

        let camera = {
            let p = if keep_camera_pixel_aligned {
                camera_offset_pixel_aligned
            } else {
                camera_offset_ideal
            };
            let rect = Rect::new(p.x, p.y, canvas_size.x, canvas_size.y);
            Camera2D {
                target: vec2(rect.x + rect.w / 2., rect.y + rect.h / 2.),
                zoom: vec2(1. / rect.w * 2., -1. / rect.h * 2.),
                offset: vec2(0., 0.),
                rotation: 0.,

                render_target: None,
                viewport: None,
            }
        };
        set_camera(&camera);

        clear_background(Color::new(0.0, 0.0, 50.0, 0.0));

        draw_circle(65.0, 50., 20.0, WHITE);
        draw_circle(130.0, 65., 35.0, BLUE);

        // draw a rectangle grid
        let loop_max = 10 * scale as usize;
        for x in (0..loop_max).step_by(scale as usize * 2) {
            for y in (0..loop_max).step_by(scale as usize * 2) {
                draw_rectangle(
                    x as f32,
                    y as f32,
                    scale,
                    scale,
                    Color::new(
                        x as f32 / loop_max as f32,
                        y as f32 / loop_max as f32,
                        (x + y) as f32 / 20.0,
                        1.0,
                    ),
                );
            }
        }

        draw_circle(virtual_mouse_pos.x, virtual_mouse_pos.y, 5.0, BLACK);

        draw_circle(15.0 + (10. * timer.cos()), 40., 5.0, ORANGE);

        {
            //crosshair
            draw_line(
                virtual_mouse_pos.x,
                virtual_mouse_pos.y - 20.0,
                virtual_mouse_pos.x,
                virtual_mouse_pos.y + 20.0,
                1.0,
                RED,
            );

            draw_line(
                virtual_mouse_pos.x - 20.0,
                virtual_mouse_pos.y,
                virtual_mouse_pos.x + 20.0,
                virtual_mouse_pos.y,
                1.0,
                RED,
            );
            draw_circle(virtual_mouse_pos.x, virtual_mouse_pos.y, 5.0, BLACK);
        }

        let font_size = 16.0 * scale;
        draw_text(
            "Hello Pixel IM-perfect",
            20.0,
            30.0 + font_size,
            font_size,
            DARKGRAY,
        );

        // ------------------------------------------------------------------------
        // Begin drawing the window screen
        // ------------------------------------------------------------------------

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

        {
            enum Trans {
                Fixed,
                LeftRight,
                UpDown,
                Circle,
            }

            enum Scale {
                Fixed,
                Size,
                SizeAndRotation,
                Rotation,
            }

            let chicken_tex_size = chicken_tex.size();
            let scale_sets = [
                Scale::Fixed,
                Scale::Size,
                Scale::SizeAndRotation,
                Scale::Rotation,
            ];
            let trans_sets = [Trans::Fixed, Trans::LeftRight, Trans::UpDown, Trans::Circle];
            let trans_dist = 10.0 * scale;
            let spacing = 34.0 * scale;

            let max_width = trans_sets.len() as f32 * spacing;
            let max_height = scale_sets.len() as f32 * spacing;
            for (i, _trans_set) in trans_sets.iter().enumerate() {
                draw_rectangle(spacing * i as f32, spacing, 1.0, max_height, GRAY);
                for (j, _scale_set) in scale_sets.iter().enumerate() {
                    draw_rectangle(0.0, spacing * (j + 1) as f32, max_width, 1.0, GRAY);
                }
            }

            for (i, trans_set) in trans_sets.iter().enumerate() {
                for (j, scale_set) in scale_sets.iter().enumerate() {
                    let size_mult = match scale_set {
                        Scale::Fixed | Scale::Rotation => 1.0,
                        Scale::Size | Scale::SizeAndRotation => timer.cos().abs(),
                    };
                    let params = DrawTextureParams {
                        dest_size: Some(chicken_tex_size * size_mult * scale),
                        rotation: match scale_set {
                            Scale::Fixed | Scale::Size => 0.0,
                            Scale::SizeAndRotation | Scale::Rotation => PI * timer.cos(),
                        },
                        ..Default::default()
                    };

                    let (x, y) = match trans_set {
                        Trans::Fixed => (0., 0.),
                        Trans::LeftRight => (timer.cos() * trans_dist, 0.),
                        Trans::UpDown => (0., timer.cos() * trans_dist),
                        Trans::Circle => (timer.cos() * trans_dist, timer.cos() * trans_dist),
                    };

                    draw_texture_ex(
                        &chicken_tex,
                        spacing * i as f32 + x,
                        spacing * (j + 1) as f32 + y,
                        WHITE,
                        params,
                    );
                }
            }
        }

        gl_use_default_material();

        // Draw 'render_target' to window screen, properly scaled
        // draw_texture_ex(
        //     &render_targ.texture,
        //     0.,
        //     0.,
        //     WHITE,
        //     DrawTextureParams {
        //         dest_size: Some(vec2(canvas_size.x * scale, canvas_size.y * scale)),
        //         flip_y: true, // Must flip y otherwise 'render_target' will be upside down
        //         ..Default::default()
        //     },
        // );

        next_frame().await;

        last_res_and_scale = (res, scale);
    }
}
