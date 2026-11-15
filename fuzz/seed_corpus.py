#!/usr/bin/env python3
"""Materialize the deterministic starting corpus; preserve locally discovered inputs."""

from pathlib import Path


SEEDS = {
    "protocol_framer/structured_seed": b'sony-framer-seed\n',
    "response_parser/command_address_set": bytes.fromhex("88 30 01 ff"),
    "response_parser/command_pan_tilt_drive": bytes.fromhex("81 01 06 01 18 14 03 01 ff"),
    "response_parser/command_power_on": bytes.fromhex("81 01 04 00 02 ff"),
    "response_parser/data_pan_tilt_position": bytes.fromhex("90 50 00 01 02 03 00 04 05 06 ff"),
    "response_parser/data_pan_tilt_position_zero": bytes.fromhex("90 50 00 00 00 00 00 00 00 00 ff"),
    "response_parser/data_power_off": bytes.fromhex("90 50 03 ff"),
    "response_parser/data_power_on": bytes.fromhex("90 50 02 ff"),
    "response_parser/data_zoom_position": bytes.fromhex("90 50 01 02 03 04 ff"),
    "response_parser/data_zoom_position_zero": bytes.fromhex("90 50 00 00 00 00 ff"),
    "response_parser/inquiry_pan_tilt_position": bytes.fromhex("81 09 06 12 ff"),
    "response_parser/inquiry_power": bytes.fromhex("81 09 04 00 ff"),
    "response_parser/malformed_all_terminators": bytes.fromhex("ff ff ff ff ff ff ff ff"),
    "response_parser/malformed_bad_address": bytes.fromhex("00 41 ff"),
    "response_parser/malformed_centered_level_underflow": bytes.fromhex("00 00 08 04"),
    "response_parser/malformed_embedded_nul": bytes.fromhex("90 50 00 00 ff 00"),
    "response_parser/malformed_error_truncated": bytes.fromhex("90 60 ff"),
    "response_parser/malformed_nibbles_out_of_range": bytes.fromhex("90 50 ff ff ff ff ff"),
    "response_parser/malformed_no_terminator": bytes.fromhex("90 50 02"),
    "response_parser/malformed_oversized_frame": b"\x81" + bytes(1030) + b"\xff",
    "response_parser/malformed_short_pan_tilt": bytes.fromhex("90 50 00 01 ff"),
    "response_parser/malformed_terminator_only": bytes.fromhex("ff"),
    "response_parser/malformed_trailing_terminator": bytes.fromhex("90 41 ff ff"),
    "response_parser/malformed_two_bytes": bytes.fromhex("90 ff"),
    "response_parser/network_change": bytes.fromhex("90 38 ff"),
    "response_parser/power_circuit_error": bytes.fromhex("90 50 04 ff"),
    "response_parser/reply_ack_socket_1": bytes.fromhex("90 41 ff"),
    "response_parser/reply_ack_socket_2": bytes.fromhex("90 42 ff"),
    "response_parser/reply_completion_socket_0": bytes.fromhex("90 50 ff"),
    "response_parser/reply_completion_socket_1": bytes.fromhex("90 51 ff"),
    "response_parser/reply_error_buffer_full_0x03": bytes.fromhex("90 60 03 ff"),
    "response_parser/reply_error_cancelled_0x04": bytes.fromhex("90 61 04 ff"),
    "response_parser/reply_error_no_socket_0x05": bytes.fromhex("90 61 05 ff"),
    "response_parser/reply_error_not_exec_0x41": bytes.fromhex("90 61 41 ff"),
    "response_parser/reply_error_syntax_0x02": bytes.fromhex("90 60 02 ff"),
    "response_parser/reply_from_camera_7": bytes.fromhex("f0 41 ff"),
    "response_target/source_seed": b'response-source-seed\n',
    "serial_startup/address_set_reply": bytes.fromhex("88 30 02 ff"),
    "serial_startup/network_change": bytes.fromhex("90 38 ff"),
    "serial_startup/oversized_noise": b"1" * 257 + b"\xff",
    "serial_startup/rs485_echo": bytes.fromhex("88 30 01 ff 88 30 02 ff"),
    "sony_envelope/structured_seed": b'sony-envelope-seed\n',
}


def main():
    corpus = Path(__file__).resolve().parent / "corpus"
    for name, data in SEEDS.items():
        path = corpus / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    print(f"Generated {len(SEEDS)} fuzz seeds in {corpus}")


if __name__ == "__main__":
    main()
