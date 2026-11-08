use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    time::Duration,
};

use serde_json::{Value, json};
use tempfile::TempDir;
use vestra::{BackendPreference, Editor, PrepareOptions};

mod common;

fn success(command: &mut Command) -> Output {
    let output = command.output().expect("command runs");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn ffmpeg_only_path(directory: &Path) -> PathBuf {
    let name = format!("ffmpeg{}", std::env::consts::EXE_SUFFIX);
    let ffmpeg = std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
        .map(|directory| directory.join(&name))
        .find(|path| path.is_file())
        .expect("FFmpeg is installed")
        .canonicalize()
        .expect("resolve FFmpeg executable");
    let bin = directory.join("bin");
    std::fs::create_dir(&bin).expect("isolated executable directory");
    #[cfg(unix)]
    std::os::unix::fs::symlink(ffmpeg, bin.join(&name)).expect("link FFmpeg");
    #[cfg(not(unix))]
    {
        std::fs::copy(&ffmpeg, bin.join(&name)).expect("copy FFmpeg");
        for entry in std::fs::read_dir(ffmpeg.parent().expect("FFmpeg directory"))
            .expect("FFmpeg dependencies")
        {
            let path = entry.expect("dependency entry").path();
            if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
            {
                std::fs::copy(&path, bin.join(path.file_name().expect("DLL name")))
                    .expect("copy FFmpeg DLL");
            }
        }
    }
    success(
        Command::new(bin.join(name))
            .arg("-version")
            .env("PATH", &bin),
    );
    bin
}

fn generate_media(directory: &Path) {
    success(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=16x16:r=8:d=1",
                "-c:v",
                "ffv1",
            ])
            .arg(directory.join("source.mkv")),
    );
    success(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(directory.join("tone.wav")),
    );
}

fn project(audio: bool) -> Value {
    let mut value = json!({
        "schema_version": 1,
        "output": {"path": "out.mp4", "width": 16, "height": 16,
            "frame_rate": "8/1", "background": "#000000", "quality": "preview",
            "audio": audio, "duration_mode": "automatic"},
        "assets": [{"id": "video", "type": "video", "source": "source.mkv"}],
        "visual": {"clips": [{"id": "video", "source": {"type": "video", "asset": "video"},
            "start": 0, "source_start": 0.25, "duration": 0.75,
            "layer": 0, "opacity": {"base_value": 1}}]}
    });
    if audio {
        value["assets"]
            .as_array_mut()
            .expect("assets")
            .push(json!({"id": "tone", "type": "audio", "source": "tone.wav"}));
        value["audio"] = json!({"tracks": [{"id": "music", "clips": [
            {"id": "tone", "asset": "tone", "start": 0, "trim_start": 0.25}
        ]}]});
    }
    value
}

fn write_project(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).expect("project JSON")).expect("write project");
}

#[test]
fn cli_validates_inspects_and_renders_audio_and_video_without_ffprobe() {
    let directory = TempDir::new().expect("temporary directory");
    let bin = ffmpeg_only_path(directory.path());
    generate_media(directory.path());
    for (video, audio) in [(true, false), (false, true), (true, true)] {
        let project_path = directory
            .path()
            .join(format!("project-{video}-{audio}.json"));
        let output_path = directory.path().join(format!("output-{video}-{audio}.mp4"));
        let mut value = project(audio);
        if !video {
            value["visual"]["clips"] = json!([]);
            value["assets"].as_array_mut().expect("assets").remove(0);
            value["output"]["background"] = json!("#ff0000");
        }
        write_project(&project_path, &value);
        for operation in ["validate", "inspect"] {
            let output = success(
                common::command()
                    .env("PATH", &bin)
                    .arg(operation)
                    .arg(&project_path)
                    .args(["--format", "json"]),
            );
            let report: Value = serde_json::from_slice(&output.stdout).expect("command JSON");
            assert_eq!(report["status"], "success");
            if operation == "inspect" {
                assert_eq!(report["output"]["duration"], 0.75);
                assert_eq!(report["output"]["total_frames"], 6);
                if audio {
                    let clip = &report["audio"]["tracks"][0]["clips"][0];
                    assert_eq!(clip["trim_start"], 0.25);
                    assert_eq!(clip["trim_end"], 1.0);
                    assert_eq!(clip["end"], 0.75);
                }
            }
        }
        let output = success(
            common::command()
                .env("PATH", &bin)
                .arg("render")
                .arg(&project_path)
                .arg("--output")
                .arg(&output_path)
                .args([
                    "--render-backend",
                    "cpu",
                    "--progress",
                    "none",
                    "--format",
                    "json",
                ]),
        );
        let report: Value = serde_json::from_slice(&output.stdout).expect("render JSON");
        assert_eq!(report["total_frames"], 6);
        assert_eq!(report["render_backend"], "cpu");
        assert_eq!(report["audio_present"], audio);

        let video = success(
            Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&output_path)
                .args(["-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "rgba", "-"]),
        );
        assert_eq!(video.stdout.len(), 6 * 16 * 16 * 4);
        for frame in video.stdout.chunks_exact(16 * 16 * 4) {
            assert!(frame[0] > 240 && frame[1] < 16 && frame[2] < 16);
        }
        let decoded_audio = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&output_path)
            .args([
                "-map", "0:a:0", "-ac", "1", "-ar", "48000", "-f", "f32le", "-",
            ])
            .output()
            .expect("decode published audio");
        assert_eq!(decoded_audio.status.success(), audio);
        if audio {
            let samples: Vec<_> = decoded_audio
                .stdout
                .chunks_exact(4)
                .map(|sample| f32::from_le_bytes(sample.try_into().expect("sample")))
                .collect();
            // AAC may retain up to one padded 1024-sample frame at the output end.
            assert!((36_000..=37_024).contains(&samples.len()));
            for start in [2_400, 16_800, 31_200] {
                let rms = (samples[start..start + 2_400]
                    .iter()
                    .map(|sample| sample * sample)
                    .sum::<f32>()
                    / 2_400.0)
                    .sqrt();
                assert!(rms > 0.04, "audio marker missing at sample {start}: {rms}");
            }
        }
    }
}

#[test]
fn published_audio_uses_contiguous_source_samples_despite_timestamp_gaps() {
    let directory = TempDir::new().expect("temporary directory");
    let source = directory.path().join("timestamp-gap.mka");
    success(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "aevalsrc=0.125*sin(2*PI*if(lt(t\\,0.5)\\,440\\,880)*t):s=48000:d=1",
                "-af",
                "asetpts=PTS+gte(T\\,0.5)*5/TB",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&source),
    );
    for rate in [1.0, 2.0] {
        let project_path = directory.path().join(format!("gap-{rate}.json"));
        let output_path = directory.path().join(format!("gap-{rate}.mp4"));
        let duration = 0.25 + 0.875 / rate;
        let effects = if rate > 1.0 {
            json!([{"id": "speed", "type": "playback_speed", "rate": rate}])
        } else {
            json!([])
        };
        let value = json!({
            "schema_version": 1,
            "output": {"path": "unused.mp4", "width": 16, "height": 16,
                "frame_rate": "16/1", "background": "#000000", "quality": "preview",
                "audio": true, "duration_mode": "automatic"},
            "assets": [{"id": "source", "type": "audio", "source": source}],
            "visual": {"clips": []},
            "audio": {"tracks": [{"id": "music", "clips": [{
                "id": "clip", "asset": "source", "start": 0.25, "trim_start": 0.125,
                "effects": effects
            }]}]}
        });
        write_project(&project_path, &value);
        let output = success(
            common::command()
                .arg("render")
                .arg(&project_path)
                .arg("--output")
                .arg(&output_path)
                .args([
                    "--render-backend",
                    "cpu",
                    "--progress",
                    "none",
                    "--format",
                    "json",
                ]),
        );
        let report: Value = serde_json::from_slice(&output.stdout).expect("render JSON");
        assert_eq!(report["duration"], duration);
        assert_eq!(report["total_frames"], (duration * 16.0) as u64);

        let decoded = success(
            Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&output_path)
                .args([
                    "-map", "0:a:0", "-ac", "1", "-ar", "48000", "-f", "f32le", "-",
                ]),
        );
        let samples: Vec<_> = decoded
            .stdout
            .chunks_exact(4)
            .map(|sample| f32::from_le_bytes(sample.try_into().expect("sample")))
            .collect();
        let expected_samples = (duration * 48_000.0) as usize;
        // AAC may retain one padded 1024-sample frame after the project end.
        assert!(
            (expected_samples..=expected_samples + 1_024).contains(&samples.len()),
            "rate {rate}: {} decoded samples, expected {expected_samples}",
            samples.len()
        );
        assert!(
            samples[..7_200].iter().all(|sample| sample.abs() < 0.001),
            "authored initial silence must remain"
        );
        for (source_time, frequency) in [(0.325, 440.0), (0.825, 880.0)] {
            let start = ((0.25 + (source_time - 0.125) / rate) * 48_000.0) as usize;
            let mut real = 0.0;
            let mut imaginary = 0.0;
            for (index, sample) in samples[start..start + 2_400].iter().enumerate() {
                let phase = std::f64::consts::TAU * frequency * index as f64 / 48_000.0;
                real += f64::from(*sample) * phase.cos();
                imaginary += f64::from(*sample) * phase.sin();
            }
            assert!(
                real.hypot(imaginary) / 1_200.0 > 0.04,
                "rate {rate}: {frequency} Hz marker missing at source time {source_time}"
            );
        }

        let timestamps = success(
            Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&output_path)
                .args([
                    "-map",
                    "0:a:0",
                    "-ac",
                    "1",
                    "-ar",
                    "48000",
                    "-c:a",
                    "pcm_s16le",
                    "-f",
                    "framecrc",
                    "-",
                ]),
        );
        let timestamps = String::from_utf8(timestamps.stdout).expect("frame timestamps");
        assert!(timestamps.contains("#tb 0: 1/48000"));
        let mut expected_pts = 0_i64;
        for line in timestamps
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields: Vec<_> = line.split(',').map(str::trim).collect();
            let pts: i64 = fields[2].parse().expect("frame PTS");
            let frame_samples: i64 = fields[3].parse().expect("frame sample count");
            assert_eq!(
                pts, expected_pts,
                "rate {rate}: discontinuous published audio"
            );
            expected_pts += frame_samples;
        }
        assert_eq!(expected_pts as usize, samples.len());
    }
}

#[test]
fn cli_rejects_invalid_source_ranges_and_missing_audio_without_ffprobe() {
    let directory = TempDir::new().expect("temporary directory");
    let bin = ffmpeg_only_path(directory.path());
    generate_media(directory.path());
    let project_path = directory.path().join("invalid.json");
    for (pointer, replacement, code) in [
        (
            "/visual/clips/0/source_start",
            json!(0.5),
            "VESTRA-VIDEO-TIMING",
        ),
        (
            "/audio/tracks/0/clips/0/trim_end",
            json!(1.1),
            "VESTRA-AUDIO-CLIP-SOURCE",
        ),
        (
            "/assets/1/source",
            json!("source.mkv"),
            "VESTRA-ASSET-AUDIO",
        ),
    ] {
        let mut value = project(true);
        value["audio"]["tracks"][0]["clips"][0]["trim_end"] = json!(1.0);
        *value.pointer_mut(pointer).expect("existing project field") = replacement;
        write_project(&project_path, &value);
        let output = common::command()
            .env("PATH", &bin)
            .arg("validate")
            .arg(&project_path)
            .args(["--format", "json"])
            .output()
            .expect("validation runs");
        assert!(!output.status.success());
        let report: Value = serde_json::from_slice(&output.stdout).expect("validation JSON");
        assert!(
            report["errors"]
                .as_array()
                .expect("errors")
                .iter()
                .any(|error| error["code"] == code),
            "expected {code}: {report}"
        );
    }
}

#[test]
fn sdk_prepares_video_and_audio_without_ffprobe() {
    const CHILD_PROJECT: &str = "VESTRA_NATIVE_PROBING_CHILD_PROJECT";
    if let Some(path) = std::env::var_os(CHILD_PROJECT) {
        // Windows also searches the spawning process's PATH, so check inside the child.
        let error = Command::new("ffprobe")
            .arg("-version")
            .output()
            .expect_err("ffprobe must be absent from the child environment");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        let editor = Editor::new();
        let project = editor.load_project(path).expect("load project");
        let mut prepared = editor
            .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
            .expect("prepare CPU video and audio without ffprobe");
        assert_eq!(prepared.preparation_report().frame_count(), 6);
        assert_eq!(
            prepared.preparation_report().duration(),
            Duration::from_millis(750)
        );
        for number in [0, 5] {
            let frame = prepared
                .render_frame_number(number)
                .expect("prepared video frame");
            let pixel = frame.as_bytes();
            assert!(pixel[0] > 240 && pixel[1] < 16 && pixel[2] < 16);
        }
        assert!(prepared.render_frame_number(6).is_err());
        return;
    }
    let directory = TempDir::new().expect("temporary directory");
    let bin = ffmpeg_only_path(directory.path());
    generate_media(directory.path());
    let project_path = directory.path().join("prepare.json");
    write_project(&project_path, &project(true));
    success(
        Command::new(std::env::current_exe().expect("integration test executable"))
            .args([
                "--exact",
                "sdk_prepares_video_and_audio_without_ffprobe",
                "--nocapture",
            ])
            .current_dir(common::workspace_root())
            .env("PATH", bin)
            .env(CHILD_PROJECT, project_path),
    );
}

#[test]
fn strict_audio_cli_resolves_decoded_duration_and_checks_source_bounds() {
    let directory = TempDir::new().expect("temporary directory");
    let source = directory.path().join("misleading.flac");
    success(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=sample_rate=48000:duration=0.375",
                "-c:a",
                "flac",
            ])
            .arg(&source),
    );
    let mut bytes = std::fs::read(&source).expect("FLAC fixture");
    assert_eq!(&bytes[..4], b"fLaC");
    let field = u64::from_be_bytes(bytes[18..26].try_into().expect("STREAMINFO"));
    bytes[18..26].copy_from_slice(&((field & !((1_u64 << 36) - 1)) | 480_000).to_be_bytes());
    std::fs::write(&source, bytes).expect("misleading metadata");
    let path = directory.path().join("project.json");
    let mut value = project(true);
    value["visual"]["clips"] = json!([]);
    value["assets"] = json!([{"id": "tone", "type": "audio", "source": "misleading.flac"}]);
    value["audio"]["tracks"][0]["clips"][0]["trim_start"] = json!(0);
    write_project(&path, &value);
    for (strict, expected) in [(false, 10.0), (true, 0.375)] {
        let mut command = common::command();
        command.arg("inspect").arg(&path).args(["--format", "json"]);
        if strict {
            command.arg("--strict-audio");
        }
        let output = success(&mut command);
        let report: Value = serde_json::from_slice(&output.stdout).expect("inspection JSON");
        assert_eq!(report["output"]["duration"], expected);
    }
    value["audio"]["tracks"][0]["clips"][0]["trim_end"] = json!(0.75);
    write_project(&path, &value);
    success(common::command().arg("validate").arg(&path));
    let output = common::command()
        .arg("validate")
        .arg(&path)
        .args(["--strict-audio", "--format", "json"])
        .output()
        .expect("strict validation");
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).expect("validation JSON");
    assert!(
        report["errors"]
            .as_array()
            .expect("errors")
            .iter()
            .any(|item| { item["code"] == "VESTRA-AUDIO-CLIP-SOURCE" })
    );
}

#[test]
fn fast_preparation_rejects_early_eof_before_output_and_strict_render_uses_samples() {
    use vestra::{CancellationToken, PreparedVideoRenderRequest};
    let directory = TempDir::new().expect("temporary directory");
    let source = directory.path().join("misleading.flac");
    success(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=0.375",
                "-c:a",
                "flac",
            ])
            .arg(&source),
    );
    let mut bytes = std::fs::read(&source).expect("FLAC fixture");
    let field = u64::from_be_bytes(bytes[18..26].try_into().expect("STREAMINFO"));
    bytes[18..26].copy_from_slice(&((field & !((1_u64 << 36) - 1)) | 480_000).to_be_bytes());
    std::fs::write(&source, bytes).expect("reported duration fixture");
    let mut value = project(true);
    value["visual"]["clips"] = json!([]);
    value["assets"] = json!([{"id": "tone", "type": "audio", "source": "misleading.flac"}]);
    value["audio"]["tracks"][0]["clips"][0]["trim_start"] = json!(0);
    let editor = Editor::new();
    let authored = vestra::Project::from_value(value.clone(), directory.path()).expect("project");
    let mut fast = editor
        .prepare(&authored, PrepareOptions::new(BackendPreference::Cpu))
        .expect("fast preparation");
    assert_eq!(
        fast.preparation_report().duration(),
        Duration::from_secs(10)
    );
    assert!(
        fast.preparation_report()
            .warnings()
            .iter()
            .any(|d| d.code == "VESTRA-AUDIO-DURATION-REPORTED")
    );
    let output = directory.path().join("output.mp4");
    std::fs::write(&output, b"previous output").expect("existing output");
    let error = fast
        .render_video(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect_err("decoded bounds must be checked before mixer padding");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|d| d.code == "VESTRA-AUDIO-CLIP-SOURCE")
    );
    assert_eq!(
        std::fs::read(&output).expect("preserved destination"),
        b"previous output"
    );
    assert!(
        !std::fs::read_dir(directory.path())
            .expect("directory")
            .any(|entry| entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".vestra-"))
    );

    for start in [0.375, 0.375 - 0.25 / 48_000.0] {
        let mut at_eof = value.clone();
        at_eof["audio"]["tracks"][0]["clips"][0]["trim_start"] = json!(start);
        at_eof["audio"]["tracks"][0]["clips"][0]["trim_end"] = json!(start + 1.0 / 48_000.0);
        let at_eof = vestra::Project::from_value(at_eof, directory.path()).expect("EOF selection");
        let mut prepared = editor
            .prepare(&at_eof, PrepareOptions::new(BackendPreference::Cpu))
            .expect("reported bounds permit the selection");
        let error = prepared
            .render_video(
                PreparedVideoRenderRequest::new(&output).with_overwrite(true),
                &mut |_| {},
                &CancellationToken::new(),
            )
            .expect_err("rounding tolerance cannot permit a selection starting at decoded EOF");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|d| d.code == "VESTRA-AUDIO-CLIP-SOURCE")
        );
        assert_eq!(
            std::fs::read(&output).expect("preserved destination"),
            b"previous output"
        );
    }

    let mut strict = editor
        .prepare(
            &authored,
            PrepareOptions::new(BackendPreference::Cpu).with_strict_audio(true),
        )
        .expect("strict preparation");
    assert_eq!(
        strict.preparation_report().duration(),
        Duration::from_secs_f64(0.375)
    );
    strict
        .render_video(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect("exact prepared output");
    let decoded = success(
        Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&output)
            .args([
                "-map", "0:a:0", "-ac", "1", "-ar", "48000", "-f", "f32le", "-",
            ]),
    );
    let samples: Vec<_> = decoded
        .stdout
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes(sample.try_into().expect("sample")))
        .collect();
    assert!(
        (18_000..=19_024).contains(&samples.len()),
        "AAC padding: {}",
        samples.len()
    );
    assert!(samples[2_400..16_800].iter().map(|x| x * x).sum::<f32>() / 14_400.0 > 0.004);

    // A bounded excerpt is valid even when the unused reported tail is wrong.
    value["audio"]["tracks"][0]["clips"][0]["trim_end"] = json!(0.25);
    value["audio"]["tracks"][0]["clips"][0]["start"] = json!(0.125);
    let excerpt = vestra::Project::from_value(value.clone(), directory.path()).expect("excerpt");
    let mut prepared = editor
        .prepare(&excerpt, PrepareOptions::new(BackendPreference::Cpu))
        .expect("bounded fast preparation");
    prepared
        .render_video(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect("available excerpt");
    let decoded = success(
        Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&output)
            .args([
                "-map", "0:a:0", "-ac", "1", "-ar", "48000", "-f", "f32le", "-",
            ]),
    );
    let samples: Vec<_> = decoded
        .stdout
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes(sample.try_into().expect("sample")))
        .collect();
    assert!((18_000..=19_024).contains(&samples.len()));
    assert!(samples[..4_800].iter().all(|sample| sample.abs() < 0.001));
    assert!(samples[7_200..16_800].iter().map(|x| x * x).sum::<f32>() / 9_600.0 > 0.004);
    value["output"]["audio"] = json!(false);
    value["audio"]["tracks"][0]["clips"][0]["trim_end"] = json!(0.75);
    let visual_only =
        vestra::Project::from_value(value, directory.path()).expect("visual-only project");
    let mut prepared = editor
        .prepare(&visual_only, PrepareOptions::new(BackendPreference::Cpu))
        .expect("visual-only preparation");
    let result = prepared
        .render_video(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect("unconsumed audio needs no execution scan");
    assert!(!result.audio_present);
}

#[test]
fn prepared_aac_keeps_reported_selection_and_strict_preparation_includes_padding() {
    use vestra::{CancellationToken, PreparedVideoRenderRequest};
    let directory = TempDir::new().expect("directory");
    success(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1.25",
                "-c:a",
                "aac",
            ])
            .arg(directory.path().join("source.m4a")),
    );
    let mut value = project(true);
    value["visual"]["clips"] = json!([]);
    value["assets"] = json!([{"id":"tone", "type":"audio", "source":"source.m4a"}]);
    value["audio"]["tracks"][0]["clips"][0]["start"] = json!(0.125);
    let editor = Editor::new();
    let project = vestra::Project::from_value(value, directory.path()).expect("project");
    for strict in [false, true] {
        let mut prepared = editor
            .prepare(
                &project,
                PrepareOptions::new(BackendPreference::Cpu).with_strict_audio(strict),
            )
            .expect("prepare AAC");
        let duration = prepared.preparation_report().duration().as_secs_f64();
        if strict {
            assert!(
                duration > 1.125 && duration < 1.15,
                "decoded padding: {duration}"
            );
        } else {
            assert_eq!(duration, 1.125);
        }
        let path = directory.path().join(format!("aac-{strict}.mp4"));
        prepared
            .render_video(
                PreparedVideoRenderRequest::new(&path),
                &mut |_| {},
                &CancellationToken::new(),
            )
            .expect("publish AAC");
        prepared
            .render_video(
                PreparedVideoRenderRequest::new(&path).with_overwrite(true),
                &mut |_| {},
                &CancellationToken::new(),
            )
            .expect("repeat prepared AAC output");
        let decoded = success(
            Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&path)
                .args([
                    "-map", "0:a:0", "-ac", "1", "-ar", "48000", "-f", "f32le", "-",
                ]),
        );
        let samples: Vec<_> = decoded
            .stdout
            .chunks_exact(4)
            .map(|sample| f32::from_le_bytes(sample.try_into().expect("sample")))
            .collect();
        let expected = (duration * 48_000.0).round() as usize;
        assert!(
            (expected..=expected + 1_024).contains(&samples.len()),
            "{} samples, expected {expected}",
            samples.len()
        );
        assert!(samples[..4_800].iter().all(|sample| sample.abs() < 0.001));
        assert!(
            samples[9_600..48_000]
                .iter()
                .map(|sample| sample * sample)
                .sum::<f32>()
                / 38_400.0
                > 0.004
        );
    }
}
