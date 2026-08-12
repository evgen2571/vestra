//! Renderer-neutral colour-matrix composition for evaluated basic effects.

use crate::plan::ColourTransform;

use super::EvaluatedEffect;

impl ColourTransform {
    fn then(mut self, matrix: [[f64; 3]; 3], offset: [f64; 3]) -> Self {
        let previous_matrix = self.matrix;
        let previous_offset = self.offset;
        self.matrix = multiply(matrix, previous_matrix);
        self.offset = add(multiply_vector(matrix, previous_offset), offset);
        self
    }

    #[must_use]
    pub fn from_effects(effects: impl IntoIterator<Item = EvaluatedEffect>) -> Self {
        effects
            .into_iter()
            .fold(Self::default(), |transform, effect| match effect {
                EvaluatedEffect::ColourTransform { transform } => transform,
                EvaluatedEffect::Brightness { amount } => transform.then(
                    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    [amount * 255.0; 3],
                ),
                EvaluatedEffect::Contrast { amount } => transform.then(
                    [[amount, 0.0, 0.0], [0.0, amount, 0.0], [0.0, 0.0, amount]],
                    [128.0 * (1.0 - amount); 3],
                ),
                EvaluatedEffect::Saturation { amount } => {
                    let luma = [0.2126, 0.7152, 0.0722];
                    let matrix = std::array::from_fn(|row| {
                        std::array::from_fn(|column| {
                            luma[column] * (1.0 - amount) + if row == column { amount } else { 0.0 }
                        })
                    });
                    transform.then(matrix, [0.0; 3])
                }
                EvaluatedEffect::Tint { colour, amount } => {
                    let amount = amount.clamp(0.0, 1.0);
                    transform.then(
                        [
                            [1.0 - amount, 0.0, 0.0],
                            [0.0, 1.0 - amount, 0.0],
                            [0.0, 0.0, 1.0 - amount],
                        ],
                        [
                            f64::from(colour[0]) * amount,
                            f64::from(colour[1]) * amount,
                            f64::from(colour[2]) * amount,
                        ],
                    )
                }
                _ => transform,
            })
    }
}

fn multiply(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|index| left[row][index] * right[index][column])
                .sum()
        })
    })
}

fn multiply_vector(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|row| {
        (0..3)
            .map(|column| matrix[row][column] * vector[column])
            .sum()
    })
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| left[index] + right[index])
}
