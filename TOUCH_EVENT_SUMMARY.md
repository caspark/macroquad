# Touch Events Without TouchPhase::Started - Quick Summary

## Question
Why do some touch inputs skip `TouchPhase::Started` and jump directly to `Moved` or other phases?

## Answer
**This is NOT a bug in miniquad/macroquad.** It's expected browser behavior combined with Chrome-specific quirks.

## Root Cause

### Primary: Event Listener Timing
- Event listeners are registered when the WASM app initializes
- If a touch **begins before** listeners are registered, `touchstart` is never fired
- When the touch **moves after** registration, you get `touchmove` without `touchstart`
- This is **standard browser behavior** per W3C specification

### Secondary: Chrome on Android Issues
- Chrome on Android has documented touch event inconsistencies
- More frequent occurrence than other browsers
- Related to performance optimizations and passive event listeners

## When It Happens
1. User touches screen while page/app is loading
2. Tab switching with active touch
3. App resuming from background
4. Orientation changes mid-touch
5. Any scenario where touch begins before WASM initialization completes

## Is the Current Code Correct?
**YES.** The code correctly:
- Captures all events the browser provides
- Translates phases accurately
- Manages touch state appropriately

The code does **not** filter or drop any events. It faithfully represents what the browser delivers.

## Impact
### Direct Touch Handling
- Touch IDs can appear with any phase
- No guarantee of seeing `Started` phase first
- Application code should handle "orphaned" touches defensively

### Mouse Simulation (Default Enabled)
When `simulate_mouse_with_touch = true`:
- `Moved` without `Started` = mouse motion without button down
- `Ended` without `Started` = button up without button down
- May confuse code expecting consistent button state

## Defensive Handling Pattern
```rust
// Track which touch IDs we've seen
let mut known_touches = HashSet::new();

for touch in touches() {
    if !known_touches.contains(&touch.id) {
        // First time seeing this touch, treat as implicit start
        // regardless of actual phase
        known_touches.insert(touch.id);
        handle_touch_start(touch);
    }

    match touch.phase {
        TouchPhase::Started => handle_touch_start(touch),
        TouchPhase::Moved => handle_touch_moved(touch),
        TouchPhase::Ended | TouchPhase::Cancelled => {
            handle_touch_end(touch);
            known_touches.remove(&touch.id);
        }
        _ => {}
    }
}
```

## Recommendation
**For application developers**: Handle this at the application level by treating the first event for any touch ID as an implicit "start" regardless of phase.

**For miniquad/macroquad**: Current behavior is correct. Any change would hide underlying browser behavior and add complexity without clear benefit. Consider documenting this behavior for users.

## See Also
- `TOUCH_EVENT_RESEARCH.md` - Detailed analysis and testing information
- `TOUCH_API_REFERENCE.md` - Complete touch API documentation
