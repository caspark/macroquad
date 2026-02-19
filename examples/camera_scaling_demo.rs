use macroquad::prelude::*;

#[macroquad::main("Camera Scaling Demo")]
async fn main() {
    // Same world area (100x100 units), different render target sizes
    let world_rect = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Create render targets with different aspect ratios
    let rt_square = render_target(100, 100);   // 1:1 aspect ratio (square)
    rt_square.texture.set_filter(FilterMode::Nearest);
    let rt_wide = render_target(200, 100);     // 2:1 aspect ratio (wide)
    rt_wide.texture.set_filter(FilterMode::Nearest);
    let rt_tall = render_target(100, 200);     // 1:2 aspect ratio (tall)
    rt_tall.texture.set_filter(FilterMode::Nearest);

    // All cameras look at the same world area but render to different aspect ratios
    let mut cam_square = Camera2D::from_display_rect(world_rect);
    let mut cam_wide = Camera2D::from_display_rect(world_rect);
    let mut cam_tall = Camera2D::from_display_rect(world_rect);

    cam_square.render_target = Some(rt_square.clone());
    cam_wide.render_target = Some(rt_wide.clone());
    cam_tall.render_target = Some(rt_tall.clone());

    loop {
        // Draw the same scene to all three render targets
        for (i, (camera, _rt)) in [
            (&cam_square, &rt_square),
            (&cam_wide, &rt_wide),
            (&cam_tall, &rt_tall)
        ].iter().enumerate() {
            set_camera(*camera);
            clear_background(if i == 0 { LIGHTGRAY } else if i == 1 { SKYBLUE } else { LIME });

            // Draw content that will show aspect ratio distortion clearly

            // Draw perfect circles - these will become ellipses if stretched
            draw_circle(25.0, 25.0, 8.0, RED);    // Top-left
            draw_circle(75.0, 25.0, 8.0, BLUE);   // Top-right
            draw_circle(25.0, 75.0, 8.0, YELLOW); // Bottom-left
            draw_circle(75.0, 75.0, 8.0, PURPLE); // Bottom-right
            draw_circle(50.0, 50.0, 12.0, BLACK); // Center (bigger)

            // Draw a square - this will show stretching most clearly
            draw_rectangle_lines(40.0, 40.0, 20.0, 20.0, 3.0, WHITE);

            // Draw grid lines to show distortion
            for i in 1..10 {
                let pos = i as f32 * 10.0;
                draw_line(pos, 0.0, pos, 100.0, 1.0, GRAY);   // Vertical lines
                draw_line(0.0, pos, 100.0, pos, 1.0, GRAY);   // Horizontal lines
            }

            // Draw world boundary
            draw_rectangle_lines(0.0, 0.0, 100.0, 100.0, 2.0, WHITE);
        }

        // Display all three render targets on screen
        set_default_camera();
        clear_background(DARKGRAY);

        // Show the different aspect ratio render targets
        let y_pos = 80.0;
        let spacing = 250.0;

        // Square render target (1:1 aspect ratio)
        draw_texture_ex(&rt_square.texture, 50.0, y_pos, WHITE, DrawTextureParams {
            dest_size: Some(vec2(150.0, 150.0)),
            flip_y: true,
            ..Default::default()
        });
        draw_text("Square 1:1", 50.0, y_pos + 160.0, 16.0, WHITE);
        draw_text("100x100 pixels", 50.0, y_pos + 180.0, 14.0, GRAY);
        draw_text("No distortion", 50.0, y_pos + 200.0, 14.0, GREEN);

        // Wide render target (2:1 aspect ratio)
        draw_texture_ex(&rt_wide.texture, 50.0 + spacing, y_pos, WHITE, DrawTextureParams {
            dest_size: Some(vec2(200.0, 100.0)), // Keep actual aspect ratio for display
            flip_y: true,
            ..Default::default()
        });
        draw_text("Wide 2:1", 50.0 + spacing, y_pos + 110.0, 16.0, WHITE);
        draw_text("200x100 pixels", 50.0 + spacing, y_pos + 130.0, 14.0, GRAY);
        draw_text("Horizontally stretched", 50.0 + spacing, y_pos + 150.0, 14.0, ORANGE);

        // Tall render target (1:2 aspect ratio)
        draw_texture_ex(&rt_tall.texture, 50.0 + spacing * 2.0, y_pos, WHITE, DrawTextureParams {
            dest_size: Some(vec2(100.0, 200.0)), // Keep actual aspect ratio for display
            flip_y: true,
            ..Default::default()
        });
        draw_text("Tall 1:2", 50.0 + spacing * 2.0, y_pos + 210.0, 16.0, WHITE);
        draw_text("100x200 pixels", 50.0 + spacing * 2.0, y_pos + 230.0, 14.0, GRAY);
        draw_text("Vertically stretched", 50.0 + spacing * 2.0, y_pos + 250.0, 14.0, ORANGE);

        draw_text("Same world area (100x100 units), different aspect ratios", 50.0, 20.0, 18.0, WHITE);
        draw_text("Notice how circles become ellipses and squares become rectangles!", 50.0, 40.0, 16.0, YELLOW);
        draw_text("Camera world rect doesn't match render target aspect ratio", 50.0, 320.0, 16.0, RED);

        next_frame().await;
    }
}
