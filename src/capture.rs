use glam::Vec2;
use miniquad::info;

use crate::prelude::Image;

// 680 pixels is width of website content area; 380 pixels gives 16:9 aspect ratio
pub const IDEAL_SIZE: Vec2 = Vec2::new(680., 380.);

#[derive(Debug)]
pub struct ScreenCapture {
    frame_number: usize,
    dir: String,
}

impl ScreenCapture {
    pub fn begin_capture() -> Self {
        // create capture directory if it doesn't exist
        let start = std::time::SystemTime::now();
        let since_the_epoch = start
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
        let dir = format!(
            "capture/{curr_date_and_time}/frames/",
            curr_date_and_time = since_the_epoch.as_millis()
        );
        std::fs::create_dir_all(&dir).expect("failed to create capture dir");

        info!("Screen capturing to {}", &dir);
        Self {
            frame_number: 0,
            dir,
        }
    }

    pub fn save_frame(&mut self, data: Image) {
        let filename = format!(
            "{capture_dir}/{frame_number}.png",
            capture_dir = self.dir,
            frame_number = self.frame_number,
        );
        data.export_png(&filename);
        info!("Captured frame {} to {}", self.frame_number, filename);
        self.frame_number += 1;
    }

    pub fn end_capture(&mut self) {
        info!(
            "Captured {} frames to {}",
            self.frame_number, self.dir
        );
    }
}
