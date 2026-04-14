/// Button debounce state machine for 4 GPIO buttons with INPUT_PULLUP.
///
/// Ported from AceButton behavior in C++ hmi_thread.cpp:
/// - 50ms debounce window
/// - INPUT_PULLUP: pressed = LOW (false), released = HIGH (true)
/// - keyState is a 4-bit bitmask (bit 0 = btn A, bit 1 = btn B, etc.)

/// Key state bitmask for each button index.
const KEY_BITS: [u8; 4] = [0x1, 0x2, 0x4, 0x8];

/// Default debounce time in milliseconds.
/// 15ms is enough for mechanical switches (typical bounce is 5-10ms).
const DEFAULT_DEBOUNCE_MS: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEventType {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonEvent {
    /// Button index (0-3).
    pub index: u8,
    /// Press or release.
    pub event_type: ButtonEventType,
    /// Full 4-bit key state bitmask after this event.
    pub key_state: u8,
}

#[derive(Debug, Clone, Copy)]
struct ButtonState {
    /// Last raw GPIO level read.
    raw_level: bool,
    /// Debounced stable level (true = released for INPUT_PULLUP).
    stable_level: bool,
    /// Timestamp (ms) of last raw level change.
    last_change_ms: u32,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            raw_level: true,    // INPUT_PULLUP: default high (released)
            stable_level: true, // starts released
            last_change_ms: 0,
        }
    }
}

/// Debounce state machine for 4 buttons.
///
/// Call `update()` every 10ms with raw GPIO levels and the current timestamp.
/// Returns any button events that occurred.
pub struct ButtonDebouncer {
    states: [ButtonState; 4],
    key_state: u8,
    debounce_ms: u32,
}

impl ButtonDebouncer {
    pub fn new() -> Self {
        Self {
            states: [ButtonState::default(); 4],
            key_state: 0,
            debounce_ms: DEFAULT_DEBOUNCE_MS,
        }
    }

    /// Current 4-bit key state bitmask.
    pub fn key_state(&self) -> u8 {
        self.key_state
    }

    /// Feed raw GPIO levels and current time. Returns any debounced events.
    ///
    /// `levels[i]` is the raw GPIO read for button i.
    /// INPUT_PULLUP: `true` = released (high), `false` = pressed (low).
    pub fn update(&mut self, levels: [bool; 4], now_ms: u32) -> heapless::Vec<ButtonEvent, 4> {
        let mut events = heapless::Vec::new();

        for i in 0..4 {
            let state = &mut self.states[i];
            let level = levels[i];

            // Detect raw level change
            if level != state.raw_level {
                state.raw_level = level;
                state.last_change_ms = now_ms;
            }

            // Check if stable for debounce window
            if state.raw_level != state.stable_level
                && now_ms.wrapping_sub(state.last_change_ms) >= self.debounce_ms
            {
                state.stable_level = state.raw_level;

                // INPUT_PULLUP: low = pressed, high = released
                if !state.stable_level {
                    // Pressed
                    self.key_state |= KEY_BITS[i];
                    let _ = events.push(ButtonEvent {
                        index: i as u8,
                        event_type: ButtonEventType::Pressed,
                        key_state: self.key_state,
                    });
                } else {
                    // Released
                    self.key_state &= !KEY_BITS[i];
                    let _ = events.push(ButtonEvent {
                        index: i as u8,
                        event_type: ButtonEventType::Released,
                        key_state: self.key_state,
                    });
                }
            }
        }

        events
    }
}

impl Default for ButtonDebouncer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_RELEASED: [bool; 4] = [true, true, true, true];

    fn press(levels: &mut [bool; 4], btn: usize) {
        levels[btn] = false; // INPUT_PULLUP: low = pressed
    }

    fn release(levels: &mut [bool; 4], btn: usize) {
        levels[btn] = true;
    }

    #[test]
    fn test_initial_state() {
        let db = ButtonDebouncer::new();
        assert_eq!(db.key_state(), 0);
    }

    #[test]
    fn test_no_events_when_idle() {
        let mut db = ButtonDebouncer::new();
        let events = db.update(ALL_RELEASED, 0);
        assert!(events.is_empty());
        let events = db.update(ALL_RELEASED, 100);
        assert!(events.is_empty());
    }

    #[test]
    fn test_press_after_debounce() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;
        press(&mut levels, 0);

        // First update: raw changes, no event yet
        let events = db.update(levels, 0);
        assert!(events.is_empty());

        // Before debounce window: still no event
        let events = db.update(levels, 10);
        assert!(events.is_empty());

        // At debounce window (15ms): event fires
        let events = db.update(levels, 15);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].index, 0);
        assert_eq!(events[0].event_type, ButtonEventType::Pressed);
        assert_eq!(events[0].key_state, 0x1);
    }

    #[test]
    fn test_release_after_debounce() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        // Press button 0
        press(&mut levels, 0);
        db.update(levels, 0);
        db.update(levels, 50);
        assert_eq!(db.key_state(), 0x1);

        // Release button 0
        release(&mut levels, 0);
        db.update(levels, 100);
        let events = db.update(levels, 150);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, ButtonEventType::Released);
        assert_eq!(events[0].key_state, 0x0);
    }

    #[test]
    fn test_rapid_toggle_suppressed() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        // Press
        press(&mut levels, 0);
        db.update(levels, 0);

        // Release before debounce window (15ms)
        release(&mut levels, 0);
        db.update(levels, 10);

        // Wait past debounce from the release
        let events = db.update(levels, 100);
        // Should not have fired a press event (was released before debounce)
        assert!(events.is_empty());
        assert_eq!(db.key_state(), 0);
    }

    #[test]
    fn test_simultaneous_two_buttons() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        // Press buttons 0 and 2 simultaneously
        press(&mut levels, 0);
        press(&mut levels, 2);
        db.update(levels, 0);

        let events = db.update(levels, 50);
        assert_eq!(events.len(), 2);
        assert_eq!(db.key_state(), 0x1 | 0x4);
    }

    #[test]
    fn test_sequential_button_presses() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        // Press button 1
        press(&mut levels, 1);
        db.update(levels, 0);
        db.update(levels, 50);
        assert_eq!(db.key_state(), 0x2);

        // Also press button 3
        press(&mut levels, 3);
        db.update(levels, 100);
        db.update(levels, 150);
        assert_eq!(db.key_state(), 0x2 | 0x8);

        // Release button 1
        release(&mut levels, 1);
        db.update(levels, 200);
        db.update(levels, 250);
        assert_eq!(db.key_state(), 0x8);
    }

    #[test]
    fn test_all_four_buttons() {
        let mut db = ButtonDebouncer::new();
        let levels = [false, false, false, false]; // all pressed

        db.update(levels, 0);
        let events = db.update(levels, 50);
        assert_eq!(events.len(), 4);
        assert_eq!(db.key_state(), 0xF);
    }

    #[test]
    fn test_key_state_bitmask_correct() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        // Press each button and verify its specific bit
        for btn in 0..4 {
            press(&mut levels, btn);
            db.update(levels, btn as u32 * 100);
            db.update(levels, btn as u32 * 100 + 50);
            assert_ne!(
                db.key_state() & KEY_BITS[btn],
                0,
                "button {btn} bit not set"
            );
        }
        assert_eq!(db.key_state(), 0xF);
    }

    #[test]
    fn test_event_contains_updated_key_state() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        // Press button 0
        press(&mut levels, 0);
        db.update(levels, 0);
        let events = db.update(levels, 50);
        assert_eq!(events[0].key_state, 0x1);

        // Press button 1 while 0 is still held
        press(&mut levels, 1);
        db.update(levels, 100);
        let events = db.update(levels, 150);
        assert_eq!(events[0].key_state, 0x3); // both bits set
    }

    #[test]
    fn test_no_duplicate_events() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        press(&mut levels, 0);
        db.update(levels, 0);
        db.update(levels, 50); // fires press event

        // Continued polling with same state: no more events
        let events = db.update(levels, 60);
        assert!(events.is_empty());
        let events = db.update(levels, 100);
        assert!(events.is_empty());
        let events = db.update(levels, 200);
        assert!(events.is_empty());
    }

    #[test]
    fn test_wrapping_timestamp() {
        let mut db = ButtonDebouncer::new();
        let mut levels = ALL_RELEASED;

        // Start near u32::MAX
        press(&mut levels, 0);
        db.update(levels, u32::MAX - 10);

        // Wraps around to small value — wrapping_sub handles this correctly
        let events = db.update(levels, 40);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, ButtonEventType::Pressed);
    }
}
