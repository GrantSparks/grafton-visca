//! Tests for inquiry response parsing with golden test data.
//!
//! This module tests the parsing of VISCA inquiry responses using
//! real-world response patterns from PTZ cameras.

use grafton_visca::command::{InquiryData, InquiryKind, Response};

#[test]
fn white_balance_mode_inquiry_preserves_existing_values_and_decodes_fr7_atw() {
    use grafton_visca::command::WhiteBalanceMode;

    for (wire, expected) in [
        (0x00, WhiteBalanceMode::Auto),
        (0x01, WhiteBalanceMode::Indoor),
        (0x02, WhiteBalanceMode::Outdoor),
        (0x03, WhiteBalanceMode::OnePush),
        (0x04, WhiteBalanceMode::ATW),
        (0x05, WhiteBalanceMode::Manual),
        (0x20, WhiteBalanceMode::ColorTemperature),
    ] {
        let response =
            Response::parse_with_type(&[0x90, 0x50, wire, 0xff], &InquiryKind::WhiteBalanceMode)
                .expect("supported white-balance mode must decode");
        assert!(matches!(
            response,
            Response::Inquiry(InquiryData::WhiteBalanceMode { mode }) if mode == expected
        ));
    }
}

#[test]
fn sony_fr7_profile_path_advertises_and_decodes_atw() {
    use grafton_visca::{
        command::WhiteBalanceMode,
        profiles::{GenericVisca, SonyFR7},
        ProfileSpec,
    };

    let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile");
    assert!(
        fr7.capabilities()
            .white_balance_modes
            .contains(&WhiteBalanceMode::ATW),
        "the FR7 registry advertises its typed ATW wire value"
    );
    let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic VISCA profile");
    assert!(
        !generic
            .capabilities()
            .white_balance_modes
            .contains(&WhiteBalanceMode::ATW),
        "ATW remains an FR7-specific advertised mode"
    );

    let response = Response::parse_with_profile::<SonyFR7>(
        &[0x90, 0x50, 0x04, 0xff],
        &InquiryKind::WhiteBalanceMode,
    )
    .expect("the Sony FR7 ATW inquiry reply must decode");
    assert!(matches!(
        response,
        Response::Inquiry(InquiryData::WhiteBalanceMode {
            mode: WhiteBalanceMode::ATW
        })
    ));
}

#[test]
fn test_parse_power_inquiry_responses() {
    // Power On response
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Power);
    assert!(
        result.is_ok(),
        "Failed to parse power on response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Power { on }) => {
            assert!(on, "Power should be on");
        }
        _ => panic!("Unexpected response type"),
    }

    // Power Off response
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Power);
    assert!(
        result.is_ok(),
        "Failed to parse power off response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Power { on }) => {
            assert!(!on, "Power should be off");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_pan_tilt_position_inquiry() {
    // Standard position
    let data = vec![
        0x90, 0x50, 0x00, 0x01, 0x02, 0x03, 0x00, 0x04, 0x05, 0x06, 0xFF,
    ];
    let result = Response::parse_with_type(&data, &InquiryKind::PanTiltPosition);
    assert!(
        result.is_ok(),
        "Failed to parse pan/tilt response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, 0x0123, "Pan position mismatch");
            assert_eq!(tilt, 0x0456, "Tilt position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }

    // Negative positions
    let data = vec![
        0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0E, 0x0D, 0xFF,
    ];
    let result = Response::parse_with_type(&data, &InquiryKind::PanTiltPosition);
    assert!(
        result.is_ok(),
        "Failed to parse negative pan/tilt: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, -1, "Pan should be -1");
            assert_eq!(tilt, -19, "Tilt should be -19");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_zoom_position_inquiry() {
    let data = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::ZoomPosition);
    assert!(
        result.is_ok(),
        "Failed to parse zoom response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::ZoomPosition { position }) => {
            assert_eq!(position, 0x1234, "Zoom position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }

    // Max zoom position
    let data = vec![0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::ZoomPosition);
    assert!(result.is_ok(), "Failed to parse max zoom: {:?}", result);

    match result.unwrap() {
        Response::Inquiry(InquiryData::ZoomPosition { position }) => {
            assert_eq!(position, 0xFFFF, "Should be max zoom");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_focus_position_inquiry() {
    let data = vec![0x90, 0x50, 0x05, 0x06, 0x07, 0x08, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::FocusPosition);
    assert!(
        result.is_ok(),
        "Failed to parse focus response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::FocusPosition { position }) => {
            assert_eq!(position, 0x5678, "Focus position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_backlight_inquiry() {
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Backlight);
    assert!(
        result.is_ok(),
        "Failed to parse backlight response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Backlight { status }) => {
            assert!(status, "Backlight should be on");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_image_flip_inquiry() {
    // Both flips on
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::FlipState);
    assert!(
        result.is_ok(),
        "Failed to parse image flip response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::FlipState {
            vertical,
            horizontal,
        }) => {
            assert!(vertical, "Vertical flip should be on");
            assert!(horizontal, "Horizontal flip should be on");
        }
        _ => panic!("Unexpected response type"),
    }

    // Only vertical flip
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::FlipState);
    assert!(
        result.is_ok(),
        "Failed to parse image flip response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::FlipState {
            vertical,
            horizontal,
        }) => {
            assert!(vertical, "Vertical flip should be on");
            assert!(!horizontal, "Horizontal flip should be off");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_brightness_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Brightness);
    assert!(
        result.is_ok(),
        "Failed to parse brightness response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Brightness { position }) => {
            assert_eq!(position, 0x08, "Brightness position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_gain_level_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x05, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Gain);
    assert!(
        result.is_ok(),
        "Failed to parse gain response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Gain { gain }) => {
            assert_eq!(gain, 0x05, "Gain value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_contrast_inquiry() {
    // Response format: y0 50 00 00 0p 0q FF where pq = position
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Contrast);
    assert!(
        result.is_ok(),
        "Failed to parse contrast response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Contrast { level }) => {
            assert_eq!(level, 0x0C, "Contrast value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_luminance_inquiry() {
    // Response format: y0 50 00 00 0p 0q FF where pq = position
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Luminance);
    assert!(
        result.is_ok(),
        "Failed to parse luminance response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Luminance { level }) => {
            assert_eq!(level, 0x07, "Luminance value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_sharpness_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Sharpness);
    assert!(
        result.is_ok(),
        "Failed to parse sharpness response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Sharpness { value }) => {
            assert_eq!(value, 0x08, "Sharpness value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_iris_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0A, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Iris);
    assert!(
        result.is_ok(),
        "Failed to parse iris response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Iris { position }) => {
            assert_eq!(position, 0x0A, "Iris position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_iris_inquiry_preserves_both_position_nibbles() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x0E, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Iris);
    assert!(
        result.is_ok(),
        "Failed to parse multi-nibble iris response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Iris { position }) => {
            assert_eq!(position, 0x1E, "Iris position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_shutter_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Shutter);
    assert!(
        result.is_ok(),
        "Failed to parse shutter response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Shutter { position }) => {
            assert_eq!(position, 0x07, "Shutter position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_noise_reduction_2d_inquiry() {
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::NoiseReduction2D);
    assert!(
        result.is_ok(),
        "Failed to parse 2D NR response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::NoiseReduction2D { level }) => {
            assert_eq!(level, 0x03, "2D NR level mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_noise_reduction_3d_inquiry() {
    let data = vec![0x90, 0x50, 0x05, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::NoiseReduction3D);
    assert!(
        result.is_ok(),
        "Failed to parse 3D NR response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::NoiseReduction3D { level }) => {
            assert_eq!(level, 0x05, "3D NR level mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_noise_reduction_3d_inquiry_full_public_domain() {
    for wire in 0x00..=0x08 {
        let response =
            Response::parse_with_type(&[0x90, 0x50, wire, 0xFF], &InquiryKind::NoiseReduction3D)
                .expect("every public 3D NR level must parse");

        assert!(matches!(
            response,
            Response::Inquiry(InquiryData::NoiseReduction3D { level }) if level == wire
        ));
    }
}

#[test]
fn test_parse_saturation_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0A, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Saturation);
    assert!(
        result.is_ok(),
        "Failed to parse saturation response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Saturation { level }) => {
            assert_eq!(level, 0x0A, "Saturation level mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_hue_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::Hue);
    assert!(result.is_ok(), "Failed to parse hue response: {:?}", result);

    match result.unwrap() {
        Response::Inquiry(InquiryData::Hue { hue }) => {
            assert_eq!(hue, 0x07, "Hue value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_auto_white_balance_sensitivity_inquiry() {
    use grafton_visca::command::AutoWhiteBalanceSensitivity;

    // Test High sensitivity (0x00)
    let data = vec![0x90, 0x50, 0x00, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::AutoWhiteBalanceSensitivity);
    assert!(
        result.is_ok(),
        "Failed to parse AWB sensitivity High response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::AutoWhiteBalanceSensitivity { sensitivity }) => {
            assert_eq!(
                sensitivity,
                AutoWhiteBalanceSensitivity::High,
                "Should be High sensitivity"
            );
        }
        _ => panic!("Unexpected response type"),
    }

    // Test Normal sensitivity (0x01)
    let data = vec![0x90, 0x50, 0x01, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::AutoWhiteBalanceSensitivity);
    assert!(
        result.is_ok(),
        "Failed to parse AWB sensitivity Normal response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::AutoWhiteBalanceSensitivity { sensitivity }) => {
            assert_eq!(
                sensitivity,
                AutoWhiteBalanceSensitivity::Normal,
                "Should be Normal sensitivity"
            );
        }
        _ => panic!("Unexpected response type"),
    }

    // Test Low sensitivity (0x02)
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::AutoWhiteBalanceSensitivity);
    assert!(
        result.is_ok(),
        "Failed to parse AWB sensitivity Low response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::AutoWhiteBalanceSensitivity { sensitivity }) => {
            assert_eq!(
                sensitivity,
                AutoWhiteBalanceSensitivity::Low,
                "Should be Low sensitivity"
            );
        }
        _ => panic!("Unexpected response type"),
    }

    // Test invalid value
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::AutoWhiteBalanceSensitivity);
    assert!(
        result.is_err(),
        "Should fail with invalid AWB sensitivity value"
    );
}

#[test]
fn test_parse_tally_red_inquiry() {
    // Tally Red On (baseline VISCA)
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::TallyRed);
    assert!(
        result.is_ok(),
        "Failed to parse tally red on response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::TallyRed { on }) => {
            assert!(on, "Tally red should be on");
        }
        _ => panic!("Unexpected response type"),
    }

    // Tally Red Off
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::TallyRed);
    assert!(
        result.is_ok(),
        "Failed to parse tally red off response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::TallyRed { on }) => {
            assert!(!on, "Tally red should be off");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_tally_green_inquiry() {
    // Tally Green On (Sony FR7)
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::TallyGreen);
    assert!(
        result.is_ok(),
        "Failed to parse tally green on response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::TallyGreen { on }) => {
            assert!(on, "Tally green should be on");
        }
        _ => panic!("Unexpected response type"),
    }

    // Tally Green Off
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::TallyGreen);
    assert!(
        result.is_ok(),
        "Failed to parse tally green off response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::TallyGreen { on }) => {
            assert!(!on, "Tally green should be off");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_tally_status_inquiry() {
    // Combined Tally Status (PTZOptics vendor extension)
    // Red off (0x02), Green on (0x03)
    let data = vec![0x90, 0x50, 0x02, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::TallyStatus);
    assert!(
        result.is_ok(),
        "Failed to parse tally status response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::TallyStatus { red_on, green_on }) => {
            assert!(!red_on, "Red tally should be off");
            assert!(green_on, "Green tally should be on");
        }
        _ => panic!("Unexpected response type"),
    }

    // Both on (0x03, 0x03)
    let data = vec![0x90, 0x50, 0x03, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::TallyStatus);
    assert!(
        result.is_ok(),
        "Failed to parse tally status both on: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::TallyStatus { red_on, green_on }) => {
            assert!(red_on, "Red tally should be on");
            assert!(green_on, "Green tally should be on");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_error_handling() {
    // Test missing terminator
    let data = vec![0x90, 0x50, 0x02];
    let result = Response::parse_with_type(&data, &InquiryKind::Power);
    assert!(result.is_err(), "Should fail without terminator");

    // Test insufficient data for pan/tilt
    let data = vec![0x90, 0x50, 0x00, 0x01, 0xFF];
    let result = Response::parse_with_type(&data, &InquiryKind::PanTiltPosition);
    assert!(result.is_err(), "Should fail with insufficient data");

    // Test empty data
    let data = vec![];
    let result = Response::parse_with_type(&data, &InquiryKind::Power);
    assert!(result.is_err(), "Should fail with empty data");
}
