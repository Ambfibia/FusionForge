
pub(super) fn identity_error(matrix: [[f64; 4]; 4]) -> f64 {
    let mut error = 0.0;
    for row in 0..4 {
        for col in 0..4 {
            let expected = if row == col { 1.0 } else { 0.0 };
            error += (matrix[row][col] - expected).abs();
        }
    }
    error
}
