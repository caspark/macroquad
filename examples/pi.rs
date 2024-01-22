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
    let chicken_tex = load_texture("examples/chicken.png").await.unwrap();
    chicken_tex.set_filter(FilterMode::Linear);

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

    let mut scale = 4.0; // rendering scale
    let mut camera_position = vec2(0., 0.); // camera position
    let mut freelook = true;

    let mut timer = 0.;
    let dt = 1. / 60.;
    let mut camera_angle = 0.0;

    let mut use_shader = true;
    let mut keep_camera_pixel_aligned = false;

    loop {
        timer += dt;

        // scale controls
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

        // rendering controls
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
            println!(
                "keep_camera_pixel_aligned is now: {}",
                keep_camera_pixel_aligned
            );
        }

        {
            // camera controls
            if is_key_pressed(KeyCode::R) {
                camera_position = vec2(0., 0.);
            }

            let camera_speed = 1.1;
            if is_key_down(KeyCode::W) {
                camera_position.y -= camera_speed;
            } else if is_key_down(KeyCode::S) {
                camera_position.y += camera_speed;
            }
            if is_key_down(KeyCode::A) {
                camera_position.x -= camera_speed;
            } else if is_key_down(KeyCode::D) {
                camera_position.x += camera_speed;
            }

            if is_key_pressed(KeyCode::F) {
                freelook = !freelook;
                println!("Freelook is now: {}", freelook);
            }
            if !freelook {
                let displacement: Vec2 = Vec2::ONE * 50.0;
                camera_position =
                    -displacement + Vec2::from_angle(camera_angle).rotate(displacement);
                camera_angle += PI * dt;
            }
        }

        // Viewport calculations
        let res = vec2(screen_width(), screen_height());
        let camera = {
            let p = if keep_camera_pixel_aligned {
                camera_position.round()
            } else {
                camera_position
            };

            let rect = Rect::new(p.x, p.y, res.x, res.y);
            Camera2D {
                target: vec2(rect.x + rect.w / 2., rect.y + rect.h / 2.),
                zoom: vec2(1. / rect.w * 2., 1. / rect.h * 2.),
                offset: vec2(0., 0.),
                ..Default::default()
            }
        };
        set_camera(&camera);

        let mouse = camera.screen_to_world(vec2(mouse_position().0, mouse_position().1));

        // ------------------------------------------------------------------------
        // Begin drawing
        // ------------------------------------------------------------------------
        if use_shader {
            gl_use_material(&material);
        } else {
            gl_use_default_material();
        }

        clear_background(WHITE);

        root_ui().label(
            Some(vec2(40., 0.)),
            &format!(
                "FPS={:?}, res={:?}, scale={:?} (= or -), camera pos={:?} (F or WASD)",
                get_fps(),
                res,
                scale,
                camera_position
            ),
        );
        root_ui().label(
            Some(vec2(40., 20.)),
            &format!(
                "Shader (P): {:?}, keep camera pixel aligned (V): {:?}",
                use_shader, keep_camera_pixel_aligned
            ),
        );

        draw_circle(65.0 * scale, 50. * scale, 20.0 * scale, GREEN);
        draw_circle(130.0 * scale, 65. * scale, 35.0 * scale, BLUE);
        draw_circle(
            (15.0 + 10. * timer.cos()) * scale,
            20. * scale,
            5.0 * scale,
            ORANGE,
        );

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

        {
            //draw mouse crosshair
            draw_line(mouse.x, mouse.y - 20.0, mouse.x, mouse.y + 20.0, 1.0, RED);
            draw_line(mouse.x - 20.0, mouse.y, mouse.x + 20.0, mouse.y, 1.0, RED);
            draw_circle(mouse.x, mouse.y, 3.0, BLACK);
        }

        let font_size = 16.0 * scale;
        draw_text(
            "Hello Pixel IM-perfect",
            20.0,
            10.0 + font_size,
            font_size,
            DARKGRAY,
        );

        {
            // draw a bunch of sprites with different movement patterns to demonstrate artifacts

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
            let y_offset = 20.0 * scale;

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
                        spacing * (j) as f32 + y + y_offset,
                        WHITE,
                        params,
                    );
                }
            }
        }

        gl_use_default_material();

        next_frame().await;
    }
}
