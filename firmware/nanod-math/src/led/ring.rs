use super::types::Rgb;

pub const RING_LED_COUNT: usize = 60;

/// Fill a ring LED buffer using the halves-pointer algorithm.
///
/// LEDs before the indicator are filled with `primary_col`,
/// LEDs after the indicator are filled with `secondary_col`,
/// and the indicator LED gets `pointer_col`.
///
/// `orientation` rotates the entire output by that many LED positions.
///
/// Ported from C++ `halvesPointer()` in hmi_thread.cpp.
pub fn halves_pointer(
    buf: &mut [Rgb; RING_LED_COUNT],
    indicator: usize,
    orientation: usize,
    pointer_col: Rgb,
    primary_col: Rgb,
    secondary_col: Rgb,
) {
    let indicator = indicator.min(RING_LED_COUNT - 1);
    for i in 0..RING_LED_COUNT {
        let color = if i < indicator {
            primary_col
        } else if i == indicator {
            pointer_col
        } else {
            secondary_col
        };
        let dest = (i + orientation) % RING_LED_COUNT;
        buf[dest] = color;
    }
}

/// Map a knob position (e.g. 0..end_pos) to a ring LED index (0..59).
pub fn position_to_led_index(current_pos: u16, start_pos: u16, end_pos: u16) -> usize {
    if end_pos <= start_pos {
        return 0;
    }
    let range = end_pos - start_pos;
    let pos = current_pos.saturating_sub(start_pos).min(range);
    let index = (pos as u32 * (RING_LED_COUNT as u32 - 1)) / range as u32;
    index as usize
}

/// Map device orientation (0-3) to LED rotation offset.
/// Orientation 0 = 0°, 1 = 90° (15 LEDs), 2 = 180° (30 LEDs), 3 = 270° (45 LEDs).
pub fn orientation_to_offset(orientation: u8) -> usize {
    match orientation {
        0 => 0,
        1 => 15,
        2 => 30,
        3 => 45,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgb = Rgb::new(255, 255, 255);
    const RED: Rgb = Rgb::new(255, 0, 0);
    const BLUE: Rgb = Rgb::new(0, 0, 255);

    #[test]
    fn test_halves_pointer_at_zero() {
        let mut buf = [Rgb::default(); RING_LED_COUNT];
        halves_pointer(&mut buf, 0, 0, WHITE, RED, BLUE);
        assert_eq!(buf[0], WHITE); // pointer
        for i in 1..RING_LED_COUNT {
            assert_eq!(buf[i], BLUE, "LED {i} should be secondary");
        }
    }

    #[test]
    fn test_halves_pointer_at_end() {
        let mut buf = [Rgb::default(); RING_LED_COUNT];
        halves_pointer(&mut buf, RING_LED_COUNT - 1, 0, WHITE, RED, BLUE);
        for i in 0..RING_LED_COUNT - 1 {
            assert_eq!(buf[i], RED, "LED {i} should be primary");
        }
        assert_eq!(buf[RING_LED_COUNT - 1], WHITE); // pointer
    }

    #[test]
    fn test_halves_pointer_midpoint() {
        let mut buf = [Rgb::default(); RING_LED_COUNT];
        halves_pointer(&mut buf, 30, 0, WHITE, RED, BLUE);
        for i in 0..30 {
            assert_eq!(buf[i], RED, "LED {i} should be primary");
        }
        assert_eq!(buf[30], WHITE);
        for i in 31..RING_LED_COUNT {
            assert_eq!(buf[i], BLUE, "LED {i} should be secondary");
        }
    }

    #[test]
    fn test_halves_pointer_with_orientation() {
        let mut buf = [Rgb::default(); RING_LED_COUNT];
        halves_pointer(&mut buf, 0, 15, WHITE, RED, BLUE);
        // Indicator at logical 0, rotated by 15 → physical LED 15 is the pointer
        assert_eq!(buf[15], WHITE);
        // Physical LED 16 should be secondary (logical 1)
        assert_eq!(buf[16], BLUE);
    }

    #[test]
    fn test_halves_pointer_orientation_wraps() {
        let mut buf = [Rgb::default(); RING_LED_COUNT];
        halves_pointer(&mut buf, 50, 30, WHITE, RED, BLUE);
        // Logical 50 + offset 30 = physical 80 % 60 = 20
        assert_eq!(buf[20], WHITE);
    }

    #[test]
    fn test_halves_pointer_clamps_indicator() {
        let mut buf = [Rgb::default(); RING_LED_COUNT];
        // indicator beyond range should clamp to 59
        halves_pointer(&mut buf, 999, 0, WHITE, RED, BLUE);
        assert_eq!(buf[59], WHITE);
    }

    #[test]
    fn test_position_to_led_index_start() {
        assert_eq!(position_to_led_index(0, 0, 255), 0);
    }

    #[test]
    fn test_position_to_led_index_end() {
        assert_eq!(position_to_led_index(255, 0, 255), 59);
    }

    #[test]
    fn test_position_to_led_index_midpoint() {
        let idx = position_to_led_index(128, 0, 255);
        // 128/255 * 59 ≈ 29.6 → 29 (integer division)
        assert_eq!(idx, 29);
    }

    #[test]
    fn test_position_to_led_index_with_offset() {
        // Range 10..110, position 60 → halfway → LED 29
        let idx = position_to_led_index(60, 10, 110);
        assert_eq!(idx, 29);
    }

    #[test]
    fn test_position_to_led_index_below_start() {
        // Position below start should saturate to 0
        assert_eq!(position_to_led_index(5, 10, 110), 0);
    }

    #[test]
    fn test_position_to_led_index_zero_range() {
        assert_eq!(position_to_led_index(50, 100, 100), 0);
    }

    #[test]
    fn test_orientation_to_offset() {
        assert_eq!(orientation_to_offset(0), 0);
        assert_eq!(orientation_to_offset(1), 15);
        assert_eq!(orientation_to_offset(2), 30);
        assert_eq!(orientation_to_offset(3), 45);
        assert_eq!(orientation_to_offset(4), 0); // invalid → 0
    }
}
