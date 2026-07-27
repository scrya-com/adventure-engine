//! Bitflags for per-element draw effects (mirrors `ESlateDrawEffect`).
//!
//! Reference: SlateCore/Rendering/RenderingCommon.h:90.
//!
//! Effects are part of the batch sort key, so two elements with different
//! effects land in different batches even if everything else matches.

use bitflags::bitflags;

bitflags! {
    /// Per-element draw effects. Determines blending / tone / alpha behaviour.
    ///
    /// All flags participate in the [`crate::SortKey`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DrawEffect: u32 {
        /// No effects — straight alpha blend of source over dest.
        const NONE = 0;
        /// Source alpha is already pre-multiplied into RGB; skip the
        /// multiply step in the shader.
        const PREMULTIPLIED_ALPHA = 1 << 0;
        /// Disable blending entirely (opaque copy).
        const NO_BLENDING = 1 << 1;
        /// Invert the source alpha (used for drop-shadows).
        const INVERT_ALPHA = 1 << 2;
        /// RGB and alpha are computed separately (used by glow / inner-shadow).
        const SEGREGATED_ALPHA = 1 << 3;
        /// Apply a per-element tint *after* sampling (multiply blend).
        const TINTED = 1 << 4;
        /// Read from a gamma-space texture — convert sRGB→linear on sample.
        const FROM_SRGB = 1 << 5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_none() {
        assert_eq!(DrawEffect::default(), DrawEffect::NONE);
    }

    #[test]
    fn combine_flags() {
        let e = DrawEffect::PREMULTIPLIED_ALPHA | DrawEffect::NO_BLENDING;
        assert!(e.contains(DrawEffect::PREMULTIPLIED_ALPHA));
        assert!(e.contains(DrawEffect::NO_BLENDING));
        assert!(!e.contains(DrawEffect::TINTED));
    }

    #[test]
    fn bits_unique() {
        // Must be distinct bits so the batcher can pack them into a sort key.
        let all = [
            DrawEffect::PREMULTIPLIED_ALPHA,
            DrawEffect::NO_BLENDING,
            DrawEffect::INVERT_ALPHA,
            DrawEffect::SEGREGATED_ALPHA,
            DrawEffect::TINTED,
            DrawEffect::FROM_SRGB,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_eq!(a.bits() & b.bits(), 0, "{a:?} and {b:?} share bits");
            }
        }
    }
}
