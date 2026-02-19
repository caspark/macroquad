# Macroquad Touch API Reference

This document provides a comprehensive reference for implementing touch controls in macroquad games running on web (WASM) builds.

## Overview

Macroquad provides full multi-touch input support through a simple API. Touch events work cross-platform (web, Android, iOS, and touch-enabled PCs) and include automatic touch-to-mouse event conversion by default.

## Core Touch Types

### TouchPhase Enum

Defined in `src/input.rs:11-36`

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TouchPhase {
    Started,      // Touch began (visible for 1 frame only)
    Stationary,   // Touch held without movement (persists until moved/ended)
    Moved,        // Touch moved to new position (visible for 1 frame only)
    Ended,        // Touch released (removed at next frame end)
    Cancelled,    // Touch cancelled by system (removed at next frame end)
}
```

### Touch Struct

```rust
#[derive(Clone, Debug)]
pub struct Touch {
    pub id: u64,              // Unique identifier for the touch
    pub phase: TouchPhase,    // Current phase of the touch
    pub position: Vec2,       // Position in pixels (physical coordinates)
}
```

## Primary Touch APIs

### Getting Touch Input

```rust
use macroquad::prelude::*;

// Get all active touches with positions in pixels
let touches: Vec<Touch> = touches();

// Get all active touches with positions in normalized range [-1, 1]
let touches: Vec<Touch> = touches_local();
```

### Touch-to-Mouse Simulation

By default, touches are automatically converted to mouse events:
- `TouchPhase::Started` → `MouseButton::Left` down event
- `TouchPhase::Moved` → Mouse motion event
- `TouchPhase::Ended` → `MouseButton::Left` up event

```rust
// Check if touch-to-mouse simulation is enabled
let is_simulating: bool = is_simulating_mouse_with_touch();

// Disable touch-to-mouse simulation (to handle touch independently)
simulate_mouse_with_touch(false);

// Re-enable touch-to-mouse simulation
simulate_mouse_with_touch(true);
```

## Touch Event Lifecycle

At the end of each frame, macroquad automatically manages touch state:

1. Touches with phase `Ended` or `Cancelled` are removed from tracking
2. Touches with phase `Started` or `Moved` are changed to `Stationary`
3. When window is minimized, all touches are set to `Ended` phase

This means:
- `Started` is only visible for 1 frame
- `Moved` is only visible for 1 frame
- `Stationary` persists until next movement or end
- `Ended` and `Cancelled` are removed at next frame end

## Multi-Touch Support

Each touch has a unique `id` field that persists across frames:

```rust
use std::collections::HashMap;

let mut active_touches: HashMap<u64, Vec2> = HashMap::new();

loop {
    for touch in touches() {
        match touch.phase {
            TouchPhase::Started => {
                active_touches.insert(touch.id, touch.position);
                println!("New touch: ID {}", touch.id);
            }
            TouchPhase::Moved => {
                active_touches.insert(touch.id, touch.position);
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                active_touches.remove(&touch.id);
                println!("Touch ended: ID {}", touch.id);
            }
            TouchPhase::Stationary => {
                // Touch is held in place
            }
        }
    }

    next_frame().await;
}
```

## Complete Example

From `examples/input_touch.rs`:

```rust
use macroquad::prelude::*;

#[macroquad::main("InputTouch")]
async fn main() {
    loop {
        clear_background(LIGHTGRAY);

        for touch in touches() {
            let (fill_color, size) = match touch.phase {
                TouchPhase::Started => (GREEN, 80.0),
                TouchPhase::Stationary => (WHITE, 60.0),
                TouchPhase::Moved => (YELLOW, 60.0),
                TouchPhase::Ended => (BLUE, 80.0),
                TouchPhase::Cancelled => (BLACK, 80.0),
            };
            draw_circle(touch.position.x, touch.position.y, size, fill_color);
        }

        draw_text("touch the screen!", 20.0, 20.0, 20.0, DARKGRAY);
        next_frame().await
    }
}
```

## Mobile Device Detection

### Important Note

Macroquad does **not** provide runtime APIs to detect if running on a mobile device. However, you have these options:

### Option 1: Compile-Time Detection

```rust
#[cfg(target_arch = "wasm32")]
{
    // This code only runs in web builds
    // You're already in a web build if this compiles
}

#[cfg(target_os = "android")]
{
    // Android-specific code
}

#[cfg(target_os = "ios")]
{
    // iOS-specific code
}
```

### Option 2: Runtime Touch Capability Detection

For web builds specifically, you can infer mobile by checking for touch support:

```rust
// Check if any touches are currently active
let has_active_touches = !touches().is_empty();

// Or check if touch events are being received
let is_touch_device = {
    let mut detected = false;
    // Set a flag on first touch event
    for touch in touches() {
        if touch.phase == TouchPhase::Started {
            detected = true;
            break;
        }
    }
    detected
};
```

### Option 3: JavaScript Interop (Web Only)

For more sophisticated detection on web, you could use JavaScript interop to check:
- `window.navigator.userAgent`
- `window.navigator.maxTouchPoints`
- CSS media queries

### Recommended Approach for Web

Since you're targeting WASM only (not mobile platforms), the simplest approach is:

```rust
use macroquad::prelude::*;

struct InputMode {
    touch_enabled: bool,
}

impl InputMode {
    fn new() -> Self {
        Self { touch_enabled: false }
    }

    fn detect_input(&mut self) {
        // Enable touch mode on first touch
        if !self.touch_enabled && !touches().is_empty() {
            self.touch_enabled = true;
            println!("Touch input detected - enabling touch controls");
        }
    }
}

#[macroquad::main("Game")]
async fn main() {
    let mut input_mode = InputMode::new();

    loop {
        input_mode.detect_input();

        if input_mode.touch_enabled {
            // Show touch controls
            // Handle touch input
        } else {
            // Use keyboard/mouse controls only
        }

        next_frame().await;
    }
}
```

## Screen and Window APIs

Useful for positioning touch UI elements:

```rust
use macroquad::prelude::*;

// Get window dimensions in logical pixels
let width: f32 = screen_width();
let height: f32 = screen_height();

// Get DPI scaling factor
let dpi: f32 = screen_dpi_scale();

// Request window resize (OS may override)
request_new_screen_size(1920.0, 1080.0);

// Toggle fullscreen
set_fullscreen(true);
```

Note: Touch positions from `touches()` are in physical pixels. Use `touches_local()` for normalized coordinates in range [-1, 1].

## Platform Support

Touch input is supported on:
- **PC**: Windows/Linux/macOS (with touch-enabled hardware)
- **HTML5/WASM**: Full support (web browsers on touch devices)
- **Android**: Full support
- **iOS**: Full support

## What Macroquad Provides

- Multi-touch input tracking (per-touch ID)
- Touch phase information (Started, Moved, Stationary, Ended, Cancelled)
- Touch coordinates in pixels and normalized form
- Automatic touch-to-mouse event conversion
- Cross-platform touch support

## What Macroquad Does NOT Provide

- Runtime mobile device detection functions
- Built-in gesture recognition (swipe, pinch, rotate, tap)
- Touch pressure/force information
- Touch size information
- Platform-specific APIs (use `cfg!()` conditionals instead)

You must implement gesture recognition yourself by tracking touch IDs, positions, and time deltas.

## Advanced: Custom Gesture Example

```rust
use macroquad::prelude::*;

struct GestureDetector {
    touch_start_pos: Option<Vec2>,
    touch_start_time: f64,
}

impl GestureDetector {
    fn new() -> Self {
        Self {
            touch_start_pos: None,
            touch_start_time: 0.0,
        }
    }

    fn update(&mut self) -> Option<Gesture> {
        let touches = touches();

        if touches.is_empty() {
            self.touch_start_pos = None;
            return None;
        }

        let touch = &touches[0];

        match touch.phase {
            TouchPhase::Started => {
                self.touch_start_pos = Some(touch.position);
                self.touch_start_time = get_time();
                None
            }
            TouchPhase::Ended => {
                if let Some(start_pos) = self.touch_start_pos {
                    let delta = touch.position - start_pos;
                    let distance = delta.length();
                    let duration = get_time() - self.touch_start_time;

                    // Detect swipe vs tap
                    if distance > 50.0 && duration < 0.5 {
                        // Swipe gesture
                        let angle = delta.y.atan2(delta.x);
                        Some(Gesture::Swipe { direction: angle, distance })
                    } else if distance < 10.0 && duration < 0.3 {
                        // Tap gesture
                        Some(Gesture::Tap { position: touch.position })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None
        }
    }
}

enum Gesture {
    Tap { position: Vec2 },
    Swipe { direction: f32, distance: f32 },
}
```

## API Source File Locations

| Component | File Path | Lines |
|-----------|-----------|-------|
| Touch structs & functions | `src/input.rs` | 11-244 |
| Touch event handling | `src/lib.rs` | 617-649, 445-456 |
| Window/screen functions | `src/window.rs` | 58-72 |
| Configuration | `src/lib.rs` | 835-846 |
| Example | `examples/input_touch.rs` | 1-23 |
| Re-exports | `src/prelude.rs` | 1-30 |

## Quick Reference Cheat Sheet

```rust
// Get all touches
let touches = touches();              // Vec<Touch> in pixels
let touches = touches_local();        // Vec<Touch> in [-1, 1] range

// Touch struct fields
touch.id         // u64: unique identifier
touch.phase      // TouchPhase: Started/Moved/Stationary/Ended/Cancelled
touch.position   // Vec2: (x, y) coordinates

// Touch-to-mouse simulation
simulate_mouse_with_touch(false);     // Disable
simulate_mouse_with_touch(true);      // Enable
is_simulating_mouse_with_touch();     // Check status

// Screen info
screen_width()                         // f32: logical width
screen_height()                        // f32: logical height
screen_dpi_scale()                     // f32: DPI scaling factor
```

## Common Patterns

### Detect First Touch (Mobile Detection Proxy)

```rust
static mut TOUCH_DETECTED: bool = false;

unsafe {
    if !TOUCH_DETECTED && !touches().is_empty() {
        TOUCH_DETECTED = true;
        println!("Touch device detected!");
    }
}
```

### Track Individual Fingers

```rust
use std::collections::HashMap;

let mut fingers: HashMap<u64, Vec2> = HashMap::new();

for touch in touches() {
    match touch.phase {
        TouchPhase::Started | TouchPhase::Moved => {
            fingers.insert(touch.id, touch.position);
        }
        TouchPhase::Ended | TouchPhase::Cancelled => {
            fingers.remove(&touch.id);
        }
        _ => {}
    }
}
```

### Disable Touch-to-Mouse for Pure Touch Handling

```rust
fn main() {
    // Disable automatic mouse simulation to handle touches independently
    simulate_mouse_with_touch(false);

    loop {
        // Now touches() won't affect is_mouse_button_down() or mouse_position()
        for touch in touches() {
            // Handle pure touch input
        }

        next_frame().await;
    }
}
```
