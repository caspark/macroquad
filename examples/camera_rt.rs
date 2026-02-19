use macroquad::prelude::*;

#[macroquad::main("Camera + Render Target Example")]
async fn main() {
    // Create a 200x200 render target
    let size = Vec2::new(200.0, 200.0);
    let render_target = render_target(size.x as u32, size.y as u32);
    render_target.texture.set_filter(FilterMode::Nearest);

    // Create camera that views world coordinates 400,400 to 600,600
    let world_rect = Rect::new(400.0, 400.0, size.x, size.y);  // x, y, width, height
    let mut camera = Camera2D::from_display_rect(world_rect);
    camera.render_target = Some(render_target.clone());

    loop {
        // Pan the camera left and right on a sin wave
        let time = get_time() as f32;
        let pan_amount = 50.0; // How far left/right to pan
        let pan_speed = 1.0;   // How fast to pan

        // Update camera target to pan horizontally
        camera.target.x = 500.0 + (time * pan_speed).sin() * pan_amount;

        // ------------------------------------------------------------------------
        // Render to the 200x200 texture with world coordinates 400,400 to 600,600
        // ------------------------------------------------------------------------
        set_camera(&camera);

        clear_background(LIGHTGRAY);

        // Draw a grid to make panning more visible
        for x in (300..700).step_by(50) {
            for y in (300..700).step_by(50) {
                draw_circle(x as f32, y as f32, 3.0, GRAY);
            }
        }

        // Draw sprite at world coordinates 500,500 (stationary in world)
        draw_circle(500.0, 500.0, 10.0, RED);

        // Draw a crosshair at camera target (this will always be centered)
        let target = camera.target;
        draw_line(target.x - 20.0, target.y, target.x + 20.0, target.y, 2.0, WHITE);
        draw_line(target.x, target.y - 20.0, target.x, target.y + 20.0, 2.0, WHITE);

        // Draw some reference points to show the coordinate system
        draw_circle(400.0, 400.0, 5.0, BLUE);  // Static reference point
        draw_circle(600.0, 600.0, 5.0, GREEN); // Static reference point
        draw_circle(450.0, 500.0, 4.0, YELLOW); // Left of center
        draw_circle(550.0, 500.0, 4.0, PURPLE); // Right of center

        // ------------------------------------------------------------------------
        // Display the render target on screen (scaled up to see it better)
        // ------------------------------------------------------------------------
        set_default_camera();

        clear_background(BLACK);

        // Draw the 200x200 render target scaled up to 400x400 on screen
        draw_texture_ex(
            &render_target.texture,
            50.0,  // x position on screen
            50.0,  // y position on screen
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(400.0, 400.0)), // Scale up 2x for visibility
                flip_y: true, // Flip Y when displaying render targets
                ..Default::default()
            },
        );

        // Draw some UI text
        draw_text("Camera panning on sin wave", 500.0, 50.0, 16.0, WHITE);
        draw_text(&format!("Camera target: ({:.1}, {:.1})", camera.target.x, camera.target.y), 500.0, 80.0, 16.0, WHITE);
        draw_text("Render Target: 200x200 pixels", 500.0, 110.0, 16.0, WHITE);
        draw_text("White crosshair = camera center", 500.0, 140.0, 16.0, WHITE);
        draw_text("Red circle = stationary at world 500,500", 500.0, 170.0, 16.0, WHITE);

        next_frame().await;
    }
}
