#[inline]
pub(crate) fn f32_to_u8(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "quantization intentionally uses Rust's saturating float-to-int cast"
    )]
    {
        value as u8
    }
}

#[inline]
pub(crate) fn f32_to_u16(value: f32) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "quantization intentionally uses Rust's saturating float-to-int cast"
    )]
    {
        value as u16
    }
}

#[inline]
pub(crate) fn f32_to_u32(value: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "quantization intentionally uses Rust's saturating float-to-int cast"
    )]
    {
        value as u32
    }
}

#[inline]
pub(crate) fn f32_to_i32(value: f32) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "quantization intentionally uses Rust's saturating float-to-int cast"
    )]
    {
        value as i32
    }
}

#[inline]
pub(crate) fn u32_to_f32(value: u32) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "quantization math is defined in f32 precision"
    )]
    {
        value as f32
    }
}

#[inline]
pub(crate) fn i32_to_f32(value: i32) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "quantization math is defined in f32 precision"
    )]
    {
        value as f32
    }
}

#[inline]
pub(crate) fn usize_to_f32(value: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "replication change-count heuristic is intentionally approximate"
    )]
    {
        value as f32
    }
}
