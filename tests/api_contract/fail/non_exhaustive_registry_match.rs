use grafton_visca::transport::FrameSequence;
fn main() {
    let _ = match FrameSequence::Full32(1) {
        FrameSequence::Full32(value) => value,
        FrameSequence::MaybeTruncated(value) => u32::from(value),
    };
}
//~ E0004
//~ "non-exhaustive patterns: `_` not covered"
