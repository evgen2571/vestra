//! Backend-neutral normalization of project image sizing declarations.

use crate::project::Sizing;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizingMode {
    Original,
    Fit,
    Cover,
    Scale(f64),
    Stretch { width: u32, height: u32 },
}

#[must_use]
pub const fn normalize(sizing: &Sizing) -> SizingMode {
    match sizing {
        Sizing::Original => SizingMode::Original,
        Sizing::Fit => SizingMode::Fit,
        Sizing::Cover => SizingMode::Cover,
        Sizing::Scale { scale } => SizingMode::Scale(*scale),
        Sizing::Stretch { width, height } => SizingMode::Stretch {
            width: *width,
            height: *height,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{SizingMode, normalize};
    use crate::project::Sizing;

    #[test]
    fn normalization_preserves_every_project_sizing_variant() {
        assert_eq!(normalize(&Sizing::Original), SizingMode::Original);
        assert_eq!(normalize(&Sizing::Fit), SizingMode::Fit);
        assert_eq!(normalize(&Sizing::Cover), SizingMode::Cover);
        assert_eq!(
            normalize(&Sizing::Scale { scale: 1.25 }),
            SizingMode::Scale(1.25)
        );
        assert_eq!(
            normalize(&Sizing::Stretch {
                width: 1280,
                height: 720
            }),
            SizingMode::Stretch {
                width: 1280,
                height: 720
            }
        );
    }
}
