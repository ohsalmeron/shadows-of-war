use super::state::{InputState, SowApp};

/// Camera/viewport fields for spatial audio — copy early to avoid borrow conflicts in render.
#[derive(Clone, Copy, Debug)]
pub struct SpatialAudioCtx {
    camera_x: f32,
    camera_y: f32,
    camera_zoom: f32,
    screen_w: f32,
    screen_h: f32,
}

impl SpatialAudioCtx {
    pub fn from_input(input: &InputState) -> Self {
        Self {
            camera_x: input.camera_x,
            camera_y: input.camera_y,
            camera_zoom: input.camera_zoom,
            screen_w: input.screen_w,
            screen_h: input.screen_h,
        }
    }

    pub fn params(self, wx: f32, wy: f32) -> sow_audio::SpatialSoundParams {
        sow_audio::SpatialSoundParams {
            wx,
            wy,
            camera_x: self.camera_x,
            camera_y: self.camera_y,
            camera_zoom: self.camera_zoom,
            screen_w: self.screen_w,
            screen_h: self.screen_h,
        }
    }
}

impl SowApp {
    #[inline]
    pub(crate) fn spatial_audio_ctx(&self) -> SpatialAudioCtx {
        SpatialAudioCtx::from_input(&self.input)
    }

    #[inline]
    pub(crate) fn spatial_sound_params(&self, wx: f32, wy: f32) -> sow_audio::SpatialSoundParams {
        self.spatial_audio_ctx().params(wx, wy)
    }
}
