use macroquad::{prelude::*, ui::root_ui};

const VIRTUAL_WIDTH: f32 = 1280.0;
const VIRTUAL_HEIGHT: f32 = 720.0;

#[macroquad::main("Pixel Perfect")]
async fn main() {
    // Setup 'render_target', used to hold the rendering result so we can resize it
    let mut render_targ = render_target(VIRTUAL_WIDTH as u32, VIRTUAL_HEIGHT as u32);
    render_targ.texture.set_filter(FilterMode::Nearest);

    // Setup camera for the virtual screen, that will render to 'render_target'
    let mut render_targ_cam =
        Camera2D::from_display_rect(Rect::new(0., 0., VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
    render_targ_cam.render_target = Some(render_targ.clone());

    let mut scale = 2.0;

    // Desired behavior:
    // Given scale factor, the screen width/height will not divide by it evenly always.
    // In that case, we need to find the gap.

    let mut last_res_and_scale = (Vec2::ZERO, scale);
    let mut camera_offset = vec2(0., 0.);
    let camera_speed = 0.1;

    loop {
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
            camera_offset.y -= camera_speed;
        } else if pressing(KeyCode::S) {
            camera_offset.y += camera_speed;
        }
        if pressing(KeyCode::A) {
            camera_offset.x -= camera_speed;
        } else if pressing(KeyCode::D) {
            camera_offset.x += camera_speed;
        }
        if is_key_pressed(KeyCode::R) {
            camera_offset = vec2(0., 0.);
        }

        let res = vec2(screen_width(), screen_height());
        let res_changed = res != last_res_and_scale.0 || last_res_and_scale.1 != scale;
        let leftover_pixels = vec2(res.x % scale, res.y % scale);
        let canvas_size = vec2(res.x - leftover_pixels.x, res.y - leftover_pixels.y) / scale;

        let draw_coords = vec2(
            (screen_width() - (canvas_size.x * scale)) * 0.5,
            (screen_height() - (canvas_size.y * scale)) * 0.5,
        )
        .floor(); // floor to make sure we're perfectly pixel-aligned

        if res_changed {
            render_targ = render_target(canvas_size.x as u32, canvas_size.y as u32);
            render_targ.texture.set_filter(FilterMode::Nearest);

            render_targ_cam =
                Camera2D::from_display_rect(Rect::new(0., 0., canvas_size.x, canvas_size.y));
            render_targ_cam.render_target = Some(render_targ.clone());
        }

        render_targ_cam.target = vec2(
            camera_offset.x + canvas_size.x / 2.,
            camera_offset.y + canvas_size.y / 2.,
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

        clear_background(LIGHTGRAY);

        draw_text("Hello Pixel Perfect", 20.0, 20.0, 16.0, DARKGRAY);
        draw_circle(canvas_size.x / 2.0 - 65.0, canvas_size.y / 2.0, 35.0, RED);
        draw_circle(canvas_size.x / 2.0 + 65.0, canvas_size.y / 2.0, 35.0, BLUE);
        draw_circle(
            canvas_size.x / 2.0,
            canvas_size.y / 2.0 - 65.0,
            35.0,
            YELLOW,
        );

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

        draw_circle(virtual_mouse_pos.x, virtual_mouse_pos.y, 15.0, BLACK);

        draw_rectangle_lines(
            virtual_mouse_pos.x.floor(),
            virtual_mouse_pos.y.floor(),
            100.,
            100.,
            1.0,
            RED,
        );

        // ------------------------------------------------------------------------
        // Begin drawing the window screen
        // ------------------------------------------------------------------------
        set_default_camera();

        clear_background(PURPLE); // Will be the letterbox color

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
                "draw_coords={:?}, camera_offset={:?}",
                draw_coords, camera_offset
            ),
        );

        // Draw 'render_target' to window screen, properly scaled and letterboxed
        draw_texture_ex(
            &render_targ.texture,
            draw_coords.x,
            draw_coords.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(canvas_size.x * scale, canvas_size.y * scale)),
                flip_y: true, // Must flip y otherwise 'render_target' will be upside down
                ..Default::default()
            },
        );

        next_frame().await;

        last_res_and_scale = (res, scale);
    }
}
