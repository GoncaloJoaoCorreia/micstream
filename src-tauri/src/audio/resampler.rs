use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResamplerError {
    #[error("Failed to construct Rubato resampler: {0}")]
    ConstructionError(#[from] rubato::ResamplerConstructionError),
    #[error("Resampling processing error: {0}")]
    ProcessError(#[from] rubato::ResampleError),
}

pub struct DynamicDriftResampler {
    resampler: SincFixedIn<f32>,
    current_ratio: f64,
}

impl DynamicDriftResampler {
    pub fn new(chunk_size: usize, initial_ratio: f64) -> Result<Self, ResamplerError> {
        let params = SincInterpolationParameters {
            sinc_len: 64,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };

        let resampler = SincFixedIn::<f32>::new(
            initial_ratio,
            1.05, // max relative ratio
            params,
            chunk_size,
            1, // mono
        )?;

        Ok(Self {
            resampler,
            current_ratio: initial_ratio,
        })
    }

    pub fn set_ratio(&mut self, new_ratio: f64) -> Result<(), ResamplerError> {
        self.resampler.set_resample_ratio(new_ratio, true)?;
        self.current_ratio = new_ratio;
        Ok(())
    }

    pub fn current_ratio(&self) -> f64 {
        self.current_ratio
    }

    /// Process a chunk of mono f32 samples and return resampled samples.
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, ResamplerError> {
        let channels = vec![input.to_vec()];
        let output = self.resampler.process(&channels, None)?;
        Ok(output.into_iter().next().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_resampler_process() {
        let chunk_size = 240;
        let mut resampler =
            DynamicDriftResampler::new(chunk_size, 1.0).expect("Failed to create resampler");
        let input = vec![0.5f32; chunk_size];
        let output = resampler.process(&input).expect("Failed to process");
        assert!(!output.is_empty());
    }
}
