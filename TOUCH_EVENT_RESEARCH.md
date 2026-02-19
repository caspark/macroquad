# Touch Event Research: Missing TouchPhase::Started Events

## Problem Statement
Sometimes touch inputs are received that never have `TouchPhase::Started` set - they jump directly to `TouchPhase::Moved` or other phases. This happens on Android, with higher frequency in Chrome than in other browsers.

## Current Implementation Analysis

### Touch Event Flow

#### Web/WASM Path (Browser)
1. **JavaScript (gl.js lines 1246-1277)**: Browser touch events are captured and translated:
   - `touchstart` → `SAPP_EVENTTYPE_TOUCHES_BEGAN` (10)
   - `touchmove` → `SAPP_EVENTTYPE_TOUCHES_MOVED` (11)
   - `touchend` → `SAPP_EVENTTYPE_TOUCHES_ENDED` (12)
   - `touchcancel` → `SAPP_EVENTTYPE_TOUCHES_CANCELED` (13)

2. **Rust WASM (wasm/keycodes.rs:160-173)**: Translates to TouchPhase:
   ```rust
   TOUCHES_BEGAN => TouchPhase::Started,
   TOUCHES_MOVED => TouchPhase::Moved,
   TOUCHES_ENDED => TouchPhase::Ended,
   TOUCHES_CANCELLED => TouchPhase::Cancelled,
   ```

3. **Called via** `touch()` function in wasm.rs:317-321

#### Native Android Path
1. **Java (MainActivity.java:84-142)**: Android MotionEvent is translated:
   - `ACTION_DOWN` / `ACTION_POINTER_DOWN` → phase 2
   - `ACTION_MOVE` → phase 0
   - `ACTION_UP` / `ACTION_POINTER_UP` → phase 1
   - `ACTION_CANCEL` → phase 3

2. **Rust (android.rs:566-572)**: Translates to TouchPhase:
   ```rust
   0 => TouchPhase::Moved,
   1 => TouchPhase::Ended,
   2 => TouchPhase::Started,
   3 => TouchPhase::Cancelled,
   ```

### Touch State Management (macroquad/lib.rs)

**Touch Event Handler (617-649)**:
- Directly inserts touch into HashMap with whatever phase it receives
- **No validation** that a touch ID exists before processing Moved/Ended
- No check that Started phase was received first

**Frame Cleanup (445-456)**:
- Ended/Cancelled touches are removed from HashMap
- Started/Moved touches transition to Stationary

## Root Cause Analysis

### 1. Browser Event Listener Timing Issue
**PRIMARY CAUSE**: If event listeners are registered after a touch has already begun, the browser will NOT retroactively fire `touchstart` for that touch.

**Scenario**:
1. User touches screen
2. Browser fires `touchstart` event
3. Application loads and registers event listeners
4. Finger is still on screen and moves
5. Application receives `touchmove` without ever receiving `touchstart`

This is **expected browser behavior**, not a bug.

### 2. Chrome-Specific Issues on Android

**Known Issues**:
- Chrome on Android has documented inconsistencies with touch event firing
- Some versions don't trigger `touchstart` on first page load but work after reload
- More common in Chrome than other browsers (as reported by user)

**Contributing Factors**:
- Browser flags and settings affecting touch handling
- Passive event listener behavior
- Touch-action CSS property interactions
- Chrome's touch optimization strategies

### 3. Android Input Device Variations

Android supports multiple input types that can generate touch events:
- Physical touchscreens
- "Fake touch" devices (mice, trackpads emulating touch)
- Stylus input

These different input sources may not generate identical event sequences.

### 4. Browser Default Action Prevention

The current implementation uses `event.preventDefault()` in touch handlers (gl.js:1247, 1255, 1263, 1271). While this prevents default browser behaviors like scrolling, it can interact with browser touch optimization in unexpected ways on certain Android/Chrome combinations.

## Is This a Bug?

### In the Application Code: **NO**
The miniquad/macroquad implementation correctly:
1. Captures all touch events that the browser provides
2. Translates them accurately to TouchPhase values
3. Manages touch state appropriately

The code does not filter, drop, or mishandle any events.

### In Browser Behavior: **PARTIALLY**
1. **Expected Behavior**: Not receiving `touchstart` for touches that began before event listeners were registered is standard browser behavior
2. **Browser Bug**: Chrome on Android has documented inconsistencies that make this more frequent than it should be

### In Application Architecture: **DESIGN CONSIDERATION**
The application does not account for the possibility of receiving touch events without a corresponding `Started` phase. This is a **valid architectural choice** since:
- It follows the browser's native API directly
- The browser makes no guarantees about event ordering if listeners are registered dynamically
- Defensive handling would add complexity for an edge case

## When Does This Occur?

### Common Scenarios:
1. **Page/Application Load**: User touches screen while app is loading
2. **Tab Switching**: User switches back to tab while finger is on screen
3. **Orientation Change**: Touch is active during device rotation
4. **Browser Resume**: App resumes from background with active touch
5. **Event Listener Registration**: Touch begins before listeners are attached

### Browser-Specific:
- **Chrome on Android**: More frequent due to touch event handling bugs
- **Other Browsers**: Can still occur but less frequently
- **Desktop Browsers**: Rare, mostly limited to scenario 1

## Implications for User Code

Applications using macroquad should be aware that:

1. **A touch may not have a Started phase** - defensive code should handle this
2. **Touch IDs may appear with Moved or even Ended phases first**
3. **This is more common in Chrome on Android**

### Impact on Mouse Simulation

By default, macroquad simulates mouse events from touch input (`simulate_mouse_with_touch = true`). When a touch event arrives without a `Started` phase:

**Without Started phase**:
- `TouchPhase::Moved` → Generates mouse motion event only (no button down!)
- `TouchPhase::Ended` → Generates mouse button up event (without prior button down!)

**Result**: Mouse event handlers may see button-up events without corresponding button-down events, or mouse motion without an active button.

This could cause issues for code that relies on consistent mouse button state.

Example defensive pattern:
```rust
match touch.phase {
    TouchPhase::Started => {
        // Initialize touch tracking
    }
    TouchPhase::Moved => {
        // Check if touch exists in tracking, initialize if not
        if !touch_exists {
            // Handle as implicit start
        }
    }
    TouchPhase::Ended => {
        // Clean up, but handle case where we never saw Started
    }
    // ...
}
```

## Recommendations

### For Application Developers:
1. **Expect missing Started phases** - treat first event for a touch ID as implicit start
2. **Test on Chrome Android** - this is where the issue is most pronounced
3. **Don't assume phase ordering** - validate touch existence before accessing state

### Potential Code Improvements (For Future Consideration):
1. **Add defensive check** in touch_event handler to detect "orphaned" touches
2. **Log warning** when Moved/Ended received without prior Started for that ID
3. **Synthesize Started event** for touches that begin with Moved phase
4. **Add configuration option** to enable strict mode (reject touches without Started)

## Testing and Reproduction

### How to Reproduce

1. **Build for WASM target**: `cargo build --target wasm32-unknown-unknown --example input_touch`
2. **Serve the application** and open in Chrome on Android
3. **Test scenarios**:

   **Scenario A - App Load with Touch Active**:
   - Place finger on screen
   - Navigate to the page
   - Keep finger down and move it
   - Expected: First event for that touch ID will be `Moved`, not `Started`

   **Scenario B - Tab Switch**:
   - Open app, place finger on screen
   - Switch to another tab (finger still down)
   - Switch back to app tab
   - Move finger
   - Expected: May receive `Moved` without prior `Started` for that touch

   **Scenario C - Delayed Load**:
   - Simulate slow network by throttling in DevTools
   - Touch screen while page is loading
   - Expected: Touch may start before event listeners are registered

### Verification Code

Add this to your application to detect and log orphaned touches:

```rust
use std::collections::HashSet;
use macroquad::prelude::*;

static mut KNOWN_TOUCH_IDS: Option<HashSet<u64>> = None;

fn init_touch_tracking() {
    unsafe {
        KNOWN_TOUCH_IDS = Some(HashSet::new());
    }
}

fn check_touches() {
    let known_ids = unsafe { KNOWN_TOUCH_IDS.as_mut().unwrap() };

    for touch in touches() {
        match touch.phase {
            TouchPhase::Started => {
                known_ids.insert(touch.id);
                println!("Touch {} started", touch.id);
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                if !known_ids.contains(&touch.id) {
                    println!("⚠️  WARNING: Touch {} appeared with {:?} phase without prior Started!",
                             touch.id, touch.phase);
                    known_ids.insert(touch.id);
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if !known_ids.contains(&touch.id) {
                    println!("⚠️  WARNING: Touch {} ended without ever starting!", touch.id);
                }
                known_ids.remove(&touch.id);
            }
        }
    }
}
```

## Conclusion

**This is NOT a bug in miniquad/macroquad.**

The behavior is a combination of:
1. **Standard browser behavior** (not retroactively firing events for touches that began before listener registration)
2. **Chrome on Android quirks** (documented inconsistencies in touch event handling)
3. **Application architecture** (direct pass-through of browser events without defensive handling)

The current implementation is **correct** - it faithfully represents what the browser provides. Whether to add defensive handling for this edge case is a design decision that depends on:
- How often users experience this in practice
- The complexity cost of defensive handling
- Whether the "fix" would hide underlying browser issues
- Backwards compatibility considerations

Applications can handle this at the application level by treating the first event for any touch ID as an implicit "start" regardless of its phase.

## References

### Browser Documentation
- [MDN Web Docs: Touch Events](https://developer.mozilla.org/en-US/docs/Web/API/Touch_events)
- [W3C Touch Events Specification](https://www.w3.org/TR/touch-events/)
- [Chrome Touch Event Handling](https://developer.chrome.com/blog/touch-event-handling)

### Known Issues
- [Chrome Android touch event inconsistencies](https://stackoverflow.com/questions/18863870/touchstart-event-never-be-triggered-on-android-chrome-at-the-first-time-of-page)
- [Android Compatibility Definition Document](https://source.android.com/docs/compatibility) - Touch input requirements

### Related Code
- `miniquad/js/gl.js:1246-1277` - Touch event listeners
- `miniquad/src/native/wasm/keycodes.rs:160-173` - Phase translation
- `miniquad/src/native/android.rs:558-580` - Android touch handling
- `miniquad/java/MainActivity.java:84-142` - Java MotionEvent processing
- `macroquad/src/lib.rs:617-649` - Touch event handler
- `macroquad/src/lib.rs:445-456` - Touch cleanup logic

## Additional Notes

### Why This Matters
This behavior is important to understand because:

1. **Game state management**: If you track touch state (e.g., "is this a tap or a drag?"), you need to handle touches appearing mid-gesture
2. **UI interactions**: Buttons and controls should handle touches that start outside their event handler registration
3. **Debugging confusion**: Developers might think their code is buggy when they see unexpected touch phases
4. **Cross-platform consistency**: Native Android builds use different event paths and may have different characteristics

### Browser Event Listener Behavior
From the W3C specification and browser implementations:
> "Touch event listeners only receive events for touches that begin after the listener is registered. Any touch gestures that were active before the listener was attached will not generate touchstart events for that listener."

This is by design to prevent unexpected behavior and performance issues. Browsers do not maintain a backlog of touch states to replay when listeners are registered.

### Chrome on Android Specifics
Chrome on Android has additional optimizations and behaviors:
- Touch event coalescing for performance
- Passive event listener defaults
- Interaction with scroll optimization
- Canvas-specific touch handling differences

These can combine to make missing `touchstart` events more common than in other browsers.
