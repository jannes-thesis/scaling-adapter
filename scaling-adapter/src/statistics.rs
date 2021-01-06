pub fn mean(values: &[f64]) -> f64 {
    let sum = values.iter().sum::<f64>();
    let count = values.len();
    sum / count as f64
}

pub fn std_deviation(values: &[f64]) -> f64 {
    let mean = mean(values);
    let count = values.len();
    let variance = values
        .iter()
        .map(|value| {
            let diff = mean - *value;
            diff * diff
        })
        .sum::<f64>()
        / count as f64;
    variance.sqrt()
}
