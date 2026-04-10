use super::types::{ButtonColors, Rgb};

pub const BUTTON_LED_COUNT: usize = 8;

/// Button-to-LED mapping: each button controls 2 LEDs on the 8-LED strip.
/// From C++ hmi_thread.cpp: {{3,4}, {2,5}, {1,6}, {0,7}}.
const BUTTON_LED_MAP: [[usize; 2]; 4] = [[3, 4], [2, 5], [1, 6], [0, 7]];

/// Key state bitmask for each button (from hmi_api.h).
const KEY_BITS: [u8; 4] = [0x1, 0x2, 0x4, 0x8];

/// Fill a button LED buffer based on key state and per-button colors.
///
/// Each of the 4 buttons maps to 2 LEDs. If the button is pressed
/// (bit set in `key_state`), its LEDs get the `pressed` color;
/// otherwise they get the `idle` color.
pub fn update_button_leds(
    buf: &mut [Rgb; BUTTON_LED_COUNT],
    key_state: u8,
    colors: &[ButtonColors; 4],
) {
    for btn in 0..4 {
        let pressed = (key_state & KEY_BITS[btn]) != 0;
        let color = if pressed {
            colors[btn].pressed
        } else {
            colors[btn].idle
        };
        for &led_idx in &BUTTON_LED_MAP[btn] {
            buf[led_idx] = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = Rgb::new(255, 0, 0);
    const GREEN: Rgb = Rgb::new(0, 255, 0);
    const BLUE: Rgb = Rgb::new(0, 0, 255);
    const WHITE: Rgb = Rgb::new(255, 255, 255);

    fn test_colors() -> [ButtonColors; 4] {
        [
            ButtonColors {
                idle: RED,
                pressed: WHITE,
            },
            ButtonColors {
                idle: GREEN,
                pressed: WHITE,
            },
            ButtonColors {
                idle: BLUE,
                pressed: WHITE,
            },
            ButtonColors {
                idle: Rgb::new(128, 128, 128),
                pressed: WHITE,
            },
        ]
    }

    #[test]
    fn test_all_idle() {
        let mut buf = [Rgb::default(); BUTTON_LED_COUNT];
        update_button_leds(&mut buf, 0x0, &test_colors());
        // Button 0 → LEDs 3,4 → RED
        assert_eq!(buf[3], RED);
        assert_eq!(buf[4], RED);
        // Button 1 → LEDs 2,5 → GREEN
        assert_eq!(buf[2], GREEN);
        assert_eq!(buf[5], GREEN);
        // Button 2 → LEDs 1,6 → BLUE
        assert_eq!(buf[1], BLUE);
        assert_eq!(buf[6], BLUE);
        // Button 3 → LEDs 0,7 → grey
        assert_eq!(buf[0], Rgb::new(128, 128, 128));
        assert_eq!(buf[7], Rgb::new(128, 128, 128));
    }

    #[test]
    fn test_all_pressed() {
        let mut buf = [Rgb::default(); BUTTON_LED_COUNT];
        update_button_leds(&mut buf, 0xF, &test_colors());
        for i in 0..BUTTON_LED_COUNT {
            assert_eq!(buf[i], WHITE, "LED {i} should be WHITE when all pressed");
        }
    }

    #[test]
    fn test_single_button_pressed() {
        let mut buf = [Rgb::default(); BUTTON_LED_COUNT];
        // Press button 0 only (bit 0x1)
        update_button_leds(&mut buf, 0x1, &test_colors());
        assert_eq!(buf[3], WHITE); // btn0 pressed
        assert_eq!(buf[4], WHITE);
        assert_eq!(buf[2], GREEN); // btn1 idle
        assert_eq!(buf[5], GREEN);
        assert_eq!(buf[1], BLUE); // btn2 idle
        assert_eq!(buf[6], BLUE);
    }

    #[test]
    fn test_two_buttons_pressed() {
        let mut buf = [Rgb::default(); BUTTON_LED_COUNT];
        // Press buttons 1 and 3 (0x2 | 0x8 = 0xA)
        update_button_leds(&mut buf, 0xA, &test_colors());
        assert_eq!(buf[3], RED); // btn0 idle
        assert_eq!(buf[2], WHITE); // btn1 pressed
        assert_eq!(buf[5], WHITE);
        assert_eq!(buf[1], BLUE); // btn2 idle
        assert_eq!(buf[0], WHITE); // btn3 pressed
        assert_eq!(buf[7], WHITE);
    }

    #[test]
    fn test_led_mapping_coverage() {
        // Verify every LED in the 8-LED strip is mapped to exactly one button
        let mut mapped = [false; BUTTON_LED_COUNT];
        for btn_leds in &BUTTON_LED_MAP {
            for &idx in btn_leds {
                assert!(!mapped[idx], "LED {idx} mapped twice");
                mapped[idx] = true;
            }
        }
        for (i, &m) in mapped.iter().enumerate() {
            assert!(m, "LED {i} not mapped to any button");
        }
    }
}
