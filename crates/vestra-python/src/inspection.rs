use super::*;
#[pyclass(
    name = "InspectOutput",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyInspectOutput {
    #[pyo3(get)]
    path: PathBuf,
    #[pyo3(get)]
    width: u32,
    #[pyo3(get)]
    height: u32,
    #[pyo3(get)]
    frame_rate: String,
    #[pyo3(get)]
    duration_mode: String,
    #[pyo3(get)]
    duration: f64,
    #[pyo3(get)]
    total_frames: u64,
    #[pyo3(get)]
    preview: bool,
}
#[pyclass(
    name = "InspectAssets",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyInspectAssets {
    #[pyo3(get)]
    images: usize,
    #[pyo3(get)]
    audio: usize,
}
#[pyclass(
    name = "InspectAudio",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyInspectAudio {
    #[pyo3(get)]
    track_count: usize,
    #[pyo3(get)]
    clip_count: usize,
    #[pyo3(get)]
    end: f64,
    #[pyo3(get)]
    tracks: Vec<PyInspectAudioTrack>,
    #[pyo3(get)]
    effects: Vec<PyInspectAudioEffect>,
}
#[pyclass(
    name = "InspectAudioEffect",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyInspectAudioEffect {
    #[pyo3(get)]
    id: String,
    #[pyo3(get, name = "type")]
    kind: String,
}
#[pyclass(
    name = "InspectAudioTrack",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyInspectAudioTrack {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    mute: bool,
    #[pyo3(get)]
    gain: f64,
    #[pyo3(get)]
    clips: Vec<PyInspectAudioClip>,
    #[pyo3(get)]
    effects: Vec<PyInspectAudioEffect>,
}
#[pyclass(
    name = "InspectAudioClip",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyInspectAudioClip {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    asset: String,
    #[pyo3(get)]
    start: f64,
    #[pyo3(get)]
    end: f64,
    #[pyo3(get)]
    trim_start: f64,
    #[pyo3(get)]
    trim_end: f64,
    #[pyo3(get)]
    mute: bool,
    #[pyo3(get)]
    gain: f64,
    #[pyo3(get)]
    gain_automation: Vec<PyInspectAudioGainKeyframe>,
    #[pyo3(get)]
    fade_in: f64,
    #[pyo3(get)]
    fade_out: f64,
    #[pyo3(get)]
    fade_in_curve: String,
    #[pyo3(get)]
    fade_out_curve: String,
    #[pyo3(get)]
    effects: Vec<PyInspectAudioEffect>,
}
#[pyclass(
    name = "InspectAudioGainKeyframe",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyInspectAudioGainKeyframe {
    #[pyo3(get)]
    time: f64,
    #[pyo3(get)]
    gain: f64,
    #[pyo3(get)]
    interpolation: String,
}
#[pyclass(
    name = "InspectionReport",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
pub(crate) struct PyInspectionReport {
    #[pyo3(get)]
    project: PathBuf,
    #[pyo3(get)]
    name: Option<String>,
    #[pyo3(get)]
    output: PyInspectOutput,
    #[pyo3(get)]
    assets: PyInspectAssets,
    #[pyo3(get)]
    visual_clips: usize,
    #[pyo3(get)]
    flashes: usize,
    #[pyo3(get)]
    transitions: usize,
    #[pyo3(get)]
    audio: Option<PyInspectAudio>,
    warnings: Vec<PyDiagnostic>,
}
impl From<NativeInspection> for PyInspectionReport {
    fn from(value: NativeInspection) -> Self {
        Self {
            project: value.project,
            name: value.name,
            output: PyInspectOutput {
                path: value.output.path,
                width: value.output.width,
                height: value.output.height,
                frame_rate: value.output.frame_rate,
                duration_mode: value.output.duration_mode,
                duration: value.output.duration,
                total_frames: value.output.total_frames,
                preview: value.output.preview,
            },
            assets: PyInspectAssets {
                images: value.assets.images,
                audio: value.assets.audio,
            },
            visual_clips: value.visual_clips,
            flashes: value.flashes,
            transitions: value.transitions,
            audio: value.audio.map(|a| PyInspectAudio {
                track_count: a.track_count,
                clip_count: a.clip_count,
                end: a.end,
                effects: a
                    .effects
                    .into_iter()
                    .map(|effect| PyInspectAudioEffect {
                        id: effect.id,
                        kind: effect.kind,
                    })
                    .collect(),
                tracks: a
                    .tracks
                    .into_iter()
                    .map(|track| PyInspectAudioTrack {
                        id: track.id,
                        mute: track.mute,
                        gain: track.gain,
                        effects: track
                            .effects
                            .into_iter()
                            .map(|effect| PyInspectAudioEffect {
                                id: effect.id,
                                kind: effect.kind,
                            })
                            .collect(),
                        clips: track
                            .clips
                            .into_iter()
                            .map(|clip| PyInspectAudioClip {
                                id: clip.id,
                                asset: clip.asset,
                                start: clip.start,
                                end: clip.end,
                                trim_start: clip.trim_start,
                                trim_end: clip.trim_end,
                                mute: clip.mute,
                                gain: clip.gain,
                                gain_automation: clip
                                    .gain_automation
                                    .into_iter()
                                    .map(|keyframe| PyInspectAudioGainKeyframe {
                                        time: keyframe.time,
                                        gain: keyframe.gain,
                                        interpolation: match keyframe.interpolation {
                                            vestra::AudioGainInterpolation::Linear => {
                                                "linear".to_owned()
                                            }
                                            vestra::AudioGainInterpolation::Hold => {
                                                "hold".to_owned()
                                            }
                                        },
                                    })
                                    .collect(),
                                fade_in: clip.fade_in,
                                fade_out: clip.fade_out,
                                fade_in_curve: match clip.fade_in_curve {
                                    vestra::AudioFadeCurve::Linear => "linear".to_owned(),
                                    vestra::AudioFadeCurve::EqualPower => "equal_power".to_owned(),
                                },
                                fade_out_curve: match clip.fade_out_curve {
                                    vestra::AudioFadeCurve::Linear => "linear".to_owned(),
                                    vestra::AudioFadeCurve::EqualPower => "equal_power".to_owned(),
                                },
                                effects: clip
                                    .effects
                                    .into_iter()
                                    .map(|effect| PyInspectAudioEffect {
                                        id: effect.id,
                                        kind: effect.kind,
                                    })
                                    .collect(),
                            })
                            .collect(),
                    })
                    .collect(),
            }),
            warnings: diagnostics(&value.warnings),
        }
    }
}
#[pymethods]
impl PyInspectionReport {
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}
