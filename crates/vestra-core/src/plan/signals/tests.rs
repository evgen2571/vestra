use crate::plan::signals::{
    AudioAnalysisTap, AudioFrequencyBand, AudioScalarFeature, AudioScalarSignal,
    AudioSignalContractError, ClampTransform, CompiledScalarSignal, CompiledSignalTransform,
    CubicResponseCurve, EnvelopeTransform, GainTransform, PreparedScalarSignal,
    PreparedScalarSignalError, RawScalarSignal, RemapTransform, ScalarSignalInterner,
    SignalPreparationError, SignalTransformContractError, prepare_scalar_signals,
    prepare_transformed_scalar_signal,
};
use crate::plan_audio::{
    MASTER_AUDIO_SAMPLE_RATE, master_audio_nyquist_hz, master_audio_nyquist_hz as plan_nyquist,
};

fn signal(start: u128, interval: u128, samples: &[f64]) -> PreparedScalarSignal {
    PreparedScalarSignal::new(start, interval, samples.to_vec()).expect("valid signal")
}

#[test]
fn samples_exactly_interpolates_and_clamps_boundaries() {
    let signal = signal(100, 100, &[0.0, 10.0, 20.0]);
    assert_eq!(signal.sample(100), 0.0);
    assert_eq!(signal.sample(200), 10.0);
    assert_eq!(signal.sample(300), 20.0);
    assert_eq!(signal.sample(150), 5.0);
    assert_eq!(signal.sample(0), 0.0);
    assert_eq!(signal.sample(999), 20.0);
}

#[test]
fn sampling_is_random_access_and_handles_large_timestamps() {
    let start = 1_000_000_000_000_000_000_000_u128;
    let signal = signal(start, 1_000_000_000, &[4.0, 8.0, 12.0]);
    assert_eq!(signal.sample(start + 2_000_000_000), 12.0);
    assert_eq!(signal.sample(start + 500_000_000), 6.0);
    assert_eq!(signal.sample(start + 1_200_000_000), 8.8);
    assert_eq!(signal.sample(start), 4.0);
}

#[test]
fn constant_and_malformed_signals_are_deliberate() {
    let signal = signal(20, 10, &[3.0]);
    assert_eq!(signal.sample(0), 3.0);
    assert_eq!(signal.sample(999), 3.0);
    assert!(matches!(
        PreparedScalarSignal::new(0, 1, vec![]),
        Err(PreparedScalarSignalError::Empty)
    ));
    assert!(matches!(
        PreparedScalarSignal::new(0, 0, vec![1.0]),
        Err(PreparedScalarSignalError::ZeroInterval)
    ));
    assert!(matches!(
        PreparedScalarSignal::new(0, 1, vec![f64::NAN]),
        Err(PreparedScalarSignalError::NonFiniteSample)
    ));
    assert!(matches!(
        PreparedScalarSignal::new(u128::MAX, 1, vec![1.0, 2.0]),
        Err(PreparedScalarSignalError::TimeRangeOverflow)
    ));
}

#[test]
fn interpolation_of_finite_samples_stays_finite() {
    let signal = signal(0, 2, &[f64::MAX, -f64::MAX]);
    assert_eq!(signal.sample(1), 0.0);
}

#[test]
fn master_audio_contract_has_one_rate_and_derived_nyquist() {
    assert_eq!(MASTER_AUDIO_SAMPLE_RATE, 48_000);
    assert_eq!(plan_nyquist(), 24_000.0);
    assert_eq!(master_audio_nyquist_hz(), 24_000.0);
}

#[test]
fn frequency_band_validates_and_canonicalizes_negative_zero() {
    let band = AudioFrequencyBand::new(-0.0, 100.0).expect("valid band");
    assert_eq!(band.min_hz().to_bits(), 0.0f64.to_bits());
    assert_eq!(
        AudioFrequencyBand::new(0.0, 24_000.0),
        Ok(AudioFrequencyBand::new(0.0, 24_000.0).expect("valid Nyquist band"))
    );
    assert_eq!(
        AudioFrequencyBand::new(-1.0, 100.0),
        Err(AudioSignalContractError::NegativeBand)
    );
    assert_eq!(
        AudioFrequencyBand::new(100.0, 100.0),
        Err(AudioSignalContractError::InvalidBandOrder)
    );
    assert_eq!(
        AudioFrequencyBand::new(200.0, 100.0),
        Err(AudioSignalContractError::InvalidBandOrder)
    );
    assert_eq!(
        AudioFrequencyBand::new(0.0, 24_000.1),
        Err(AudioSignalContractError::BandExceedsNyquist)
    );
    assert_eq!(
        AudioFrequencyBand::new(f64::NAN, 100.0),
        Err(AudioSignalContractError::NonFiniteBand)
    );
    assert_eq!(
        AudioFrequencyBand::new(0.0, f64::INFINITY),
        Err(AudioSignalContractError::NonFiniteBand)
    );
}

#[test]
fn interner_assigns_dense_deterministic_ids_and_requirements() {
    let band = AudioFrequencyBand::new(40.0, 160.0).expect("valid band");
    let other_band = AudioFrequencyBand::new(160.0, 500.0).expect("valid band");
    let rms = CompiledScalarSignal::raw_audio(AudioScalarSignal {
        tap: AudioAnalysisTap::Master,
        feature: AudioScalarFeature::Rms,
    });
    let peak = CompiledScalarSignal::raw_audio(AudioScalarSignal {
        tap: AudioAnalysisTap::Master,
        feature: AudioScalarFeature::Peak,
    });
    let band_signal = CompiledScalarSignal::raw_audio(AudioScalarSignal {
        tap: AudioAnalysisTap::Master,
        feature: AudioScalarFeature::BandEnergy(band),
    });
    let other_band_signal = CompiledScalarSignal::raw_audio(AudioScalarSignal {
        tap: AudioAnalysisTap::Master,
        feature: AudioScalarFeature::BandEnergy(other_band),
    });
    let mut interner = ScalarSignalInterner::default();
    let rms_id = interner.intern(rms.clone());
    assert_eq!(rms_id, interner.intern(rms.clone()));
    assert_eq!(interner.intern(peak).index(), 1);
    assert_eq!(interner.intern(band_signal.clone()).index(), 2);
    assert_eq!(interner.intern(other_band_signal.clone()).index(), 3);
    let signals = interner.finish();
    assert_eq!(signals.len(), 4);
    assert_eq!(signals.audio_analysis_requirements().iter().len(), 4);

    let duplicate_raw_work = crate::plan::signals::CompiledScalarSignals {
        signals: vec![
            rms.clone(),
            rms,
            band_signal.clone(),
            band_signal,
            other_band_signal,
        ],
    };
    let requirements = duplicate_raw_work.audio_analysis_requirements();
    assert_eq!(requirements.iter().len(), 3);
    assert!(requirements.requires_master_audio());
}

fn rms_source() -> RawScalarSignal {
    RawScalarSignal::Audio(AudioScalarSignal {
        tap: AudioAnalysisTap::Master,
        feature: AudioScalarFeature::Rms,
    })
}

#[test]
fn transforms_apply_in_declaration_order_and_remap_extrapolates() {
    let raw = signal(10, 10, &[-1.0, 0.0, 0.5, 1.0, 2.0]);
    let remap = CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 0.0, 10.0).unwrap());
    let clamped = prepare_transformed_scalar_signal(
        &raw,
        &[
            remap,
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 10.0).unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(
        prepare_transformed_scalar_signal(&raw, &[remap])
            .unwrap()
            .samples,
        [-10.0, 0.0, 5.0, 10.0, 20.0]
    );
    assert_eq!(clamped.samples, [0.0, 0.0, 5.0, 10.0, 10.0]);

    let input = signal(0, 1, &[2.0]);
    let gain_then_clamp = prepare_transformed_scalar_signal(
        &input,
        &[
            CompiledSignalTransform::Gain(GainTransform::new(3.0).unwrap()),
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 5.0).unwrap()),
        ],
    )
    .unwrap();
    let clamp_then_gain = prepare_transformed_scalar_signal(
        &input,
        &[
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 5.0).unwrap()),
            CompiledSignalTransform::Gain(GainTransform::new(3.0).unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(gain_then_clamp.samples, [5.0]);
    assert_eq!(clamp_then_gain.samples, [6.0]);
}

#[test]
fn remap_descends_and_transform_contracts_validate() {
    let raw = signal(0, 1, &[0.0, 0.25, 0.5, 0.75, 1.0]);
    let descending = prepare_transformed_scalar_signal(
        &raw,
        &[CompiledSignalTransform::Remap(
            RemapTransform::new(0.0, 1.0, 10.0, 0.0).unwrap(),
        )],
    )
    .unwrap();
    assert_eq!(descending.samples, [10.0, 7.5, 5.0, 2.5, 0.0]);
    assert_eq!(
        GainTransform::new(f64::NAN),
        Err(SignalTransformContractError::NonFiniteParameter)
    );
    assert_eq!(
        RemapTransform::new(1.0, 1.0, 0.0, 1.0),
        Err(SignalTransformContractError::InvalidRemapInputRange)
    );
    assert!(RemapTransform::new(0.0, 1.0, 5.0, 5.0).is_ok());
    assert_eq!(
        ClampTransform::new(1.0, 0.0),
        Err(SignalTransformContractError::InvalidClampRange)
    );
    assert!(ClampTransform::new(5.0, 5.0).is_ok());
}

#[test]
fn transformed_preparation_preserves_metadata_and_rejects_overflow() {
    let raw = signal(100, 10, &[0.0, 0.25, 0.5, 1.0]);
    let transformed = prepare_transformed_scalar_signal(
        &raw,
        &[
            CompiledSignalTransform::Gain(GainTransform::new(2.0).unwrap()),
            CompiledSignalTransform::Remap(RemapTransform::new(0.0, 2.0, 0.0, 10.0).unwrap()),
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 8.0).unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(transformed.start_time, raw.start_time);
    assert_eq!(transformed.sample_interval, raw.sample_interval);
    assert_eq!(transformed.samples, [0.0, 2.5, 5.0, 8.0]);
    assert_eq!(transformed.sample(130), 8.0);
    assert_eq!(transformed.sample(105), 1.25);
    assert!(matches!(
        prepare_transformed_scalar_signal(
            &signal(0, 1, &[f64::MAX]),
            &[CompiledSignalTransform::Gain(
                GainTransform::new(f64::MAX).unwrap()
            ),]
        ),
        Err(SignalPreparationError::NonFiniteTransformedSample)
    ));
}

#[test]
fn complete_signal_identity_and_raw_requirement_sharing_are_separate() {
    let gain_two = CompiledSignalTransform::Gain(GainTransform::new(2.0).unwrap());
    let clamp = CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap());
    let same_a = CompiledScalarSignal::new(rms_source(), vec![gain_two, clamp]);
    let same_b = CompiledScalarSignal::new(rms_source(), vec![gain_two, clamp]);
    let different_gain = CompiledScalarSignal::new(
        rms_source(),
        vec![CompiledSignalTransform::Gain(
            GainTransform::new(3.0).unwrap(),
        )],
    );
    let reversed = CompiledScalarSignal::new(rms_source(), vec![clamp, gain_two]);
    let zero_a = CompiledScalarSignal::new(
        rms_source(),
        vec![CompiledSignalTransform::Gain(
            GainTransform::new(-0.0).unwrap(),
        )],
    );
    let zero_b = CompiledScalarSignal::new(
        rms_source(),
        vec![CompiledSignalTransform::Gain(
            GainTransform::new(0.0).unwrap(),
        )],
    );
    let mut interner = ScalarSignalInterner::default();
    assert_eq!(interner.intern(same_a), interner.intern(same_b));
    assert_ne!(interner.intern(different_gain), interner.intern(reversed));
    assert_eq!(interner.intern(zero_a), interner.intern(zero_b));
    let signals = interner.finish();
    assert_eq!(signals.len(), 4);
    assert_eq!(signals.audio_analysis_requirements().iter().len(), 1);
}

#[test]
fn one_raw_feature_prepares_multiple_complete_signals() {
    use crate::plan::signals::{AudioAnalysisRequirement, CompiledScalarSignals};
    let raw = signal(0, 10, &[0.0, 0.5, 1.0]);
    let source = rms_source();
    let compiled = CompiledScalarSignals::from_signals(vec![
        CompiledScalarSignal::new(source, vec![]),
        CompiledScalarSignal::new(
            source,
            vec![CompiledSignalTransform::Gain(
                GainTransform::new(2.0).unwrap(),
            )],
        ),
        CompiledScalarSignal::new(
            source,
            vec![CompiledSignalTransform::Gain(
                GainTransform::new(4.0).unwrap(),
            )],
        ),
    ]);
    let prepared = prepare_scalar_signals(
        &compiled,
        std::collections::BTreeMap::from([(
            AudioAnalysisRequirement::Master(AudioScalarFeature::Rms),
            raw,
        )]),
    )
    .unwrap();
    assert_eq!(prepared.len(), 3);
    assert_eq!(prepared.signals[0].samples, [0.0, 0.5, 1.0]);
    assert_eq!(prepared.signals[1].samples, [0.0, 1.0, 2.0]);
    assert_eq!(prepared.signals[2].samples, [0.0, 2.0, 4.0]);
}

fn envelope(attack: u128, release: u128) -> CompiledSignalTransform {
    CompiledSignalTransform::Envelope(EnvelopeTransform::new(attack, release))
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

fn response_curve(x1: f64, y1: f64, x2: f64, y2: f64) -> CompiledSignalTransform {
    CompiledSignalTransform::ResponseCurve(CubicResponseCurve::new(x1, y1, x2, y2).unwrap())
}

#[test]
fn response_curve_validates_controls_and_canonicalizes_zero() {
    assert!(CubicResponseCurve::new(0.0, -0.2, 1.0, 1.2).is_ok());
    assert_eq!(
        CubicResponseCurve::new(f64::NAN, 0.0, 1.0, 1.0),
        Err(SignalTransformContractError::NonFiniteParameter)
    );
    assert_eq!(
        CubicResponseCurve::new(0.0, f64::INFINITY, 1.0, 1.0),
        Err(SignalTransformContractError::NonFiniteParameter)
    );
    assert_eq!(
        CubicResponseCurve::new(-0.1, 0.0, 1.0, 1.0),
        Err(SignalTransformContractError::InvalidResponseCurveXControls)
    );
    assert_eq!(
        CubicResponseCurve::new(0.0, 0.0, 1.1, 1.0),
        Err(SignalTransformContractError::InvalidResponseCurveXControls)
    );
    assert_eq!(
        CubicResponseCurve::new(0.75, 0.0, 0.25, 1.0),
        Err(SignalTransformContractError::InvalidResponseCurveXControls)
    );
    assert_eq!(
        CubicResponseCurve::new(-0.0, -0.0, 1.0, 0.0),
        CubicResponseCurve::new(0.0, 0.0, 1.0, -0.0)
    );
}

#[test]
fn response_curve_clamps_its_input_and_preserves_non_linear_output() {
    let identity = CubicResponseCurve::new(0.0, 0.0, 1.0, 1.0).unwrap();
    for input in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
        assert_close(identity.evaluate(input), input);
    }
    assert_eq!(identity.evaluate(-10.0), 0.0);
    assert_eq!(identity.evaluate(0.0), 0.0);
    assert_eq!(identity.evaluate(1.0), 1.0);
    assert_eq!(identity.evaluate(10.0), 1.0);

    let ease_in = CubicResponseCurve::new(0.42, 0.0, 1.0, 1.0).unwrap();
    let ease_out = CubicResponseCurve::new(0.0, 0.0, 0.58, 1.0).unwrap();
    assert!(ease_in.evaluate(0.5) < 0.5);
    assert!(ease_out.evaluate(0.5) > 0.5);

    let overshoot = CubicResponseCurve::new(0.25, -0.5, 0.75, 1.5).unwrap();
    assert!(overshoot.evaluate(0.1) < 0.0);
    assert!(overshoot.evaluate(0.9) > 1.0);
}

#[test]
fn response_curve_preserves_order_with_pointwise_and_envelope_transforms() {
    let curve = response_curve(0.42, 0.0, 1.0, 1.0);
    let raw = signal(0, 10_000_000, &[0.0, 1.0, 1.0, 1.0]);
    let curve_then_envelope =
        prepare_transformed_scalar_signal(&raw, &[curve, envelope(20_000_000, 20_000_000)])
            .unwrap();
    let envelope_then_curve =
        prepare_transformed_scalar_signal(&raw, &[envelope(20_000_000, 20_000_000), curve])
            .unwrap();
    assert_ne!(curve_then_envelope.samples, envelope_then_curve.samples);

    let input = signal(0, 1, &[0.25]);
    let remap_then_curve = prepare_transformed_scalar_signal(
        &input,
        &[
            CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 0.0, 0.5).unwrap()),
            curve,
        ],
    )
    .unwrap();
    let curve_then_remap = prepare_transformed_scalar_signal(
        &input,
        &[
            curve,
            CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 0.0, 0.5).unwrap()),
        ],
    )
    .unwrap();
    assert_ne!(remap_then_curve.samples, curve_then_remap.samples);

    let bounded = prepare_transformed_scalar_signal(
        &signal(0, 1, &[0.1, 0.9]),
        &[
            response_curve(0.25, -0.5, 0.75, 1.5),
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(bounded.samples, [0.0, 1.0]);
}

#[test]
fn response_curves_participate_in_complete_signal_identity_and_raw_sharing() {
    use crate::plan::signals::{AudioAnalysisRequirement, CompiledScalarSignals};

    let band = AudioFrequencyBand::new(40.0, 160.0).unwrap();
    let source = RawScalarSignal::Audio(AudioScalarSignal {
        tap: AudioAnalysisTap::Master,
        feature: AudioScalarFeature::BandEnergy(band),
    });
    let curve = response_curve(0.25, 0.1, 0.75, 0.9);
    let mut interner = ScalarSignalInterner::default();
    let id = interner.intern(CompiledScalarSignal::new(source, vec![curve]));
    assert_eq!(
        id,
        interner.intern(CompiledScalarSignal::new(source, vec![curve]))
    );
    assert_ne!(
        id,
        interner.intern(CompiledScalarSignal::new(
            source,
            vec![response_curve(0.25, 0.2, 0.75, 0.9)],
        ))
    );
    assert_ne!(
        id,
        interner.intern(CompiledScalarSignal::new(
            source,
            vec![envelope(20, 180), curve],
        ))
    );

    let compiled = CompiledScalarSignals::from_signals(vec![
        CompiledScalarSignal::new(source, vec![response_curve(0.25, 0.1, 0.75, 0.9)]),
        CompiledScalarSignal::new(source, vec![response_curve(0.3, 0.1, 0.75, 0.9)]),
        CompiledScalarSignal::new(source, vec![envelope(20, 180), curve]),
    ]);
    assert_eq!(compiled.audio_analysis_requirements().iter().len(), 1);
    let prepared = prepare_scalar_signals(
        &compiled,
        std::collections::BTreeMap::from([(
            AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band)),
            signal(0, 10, &[0.0, 0.5, 1.0]),
        )]),
    )
    .unwrap();
    assert_eq!(prepared.len(), 3);
}

#[test]
fn envelope_uses_elapsed_time_and_preserves_its_initial_input() {
    let attack = prepare_transformed_scalar_signal(
        &signal(0, 20_000_000, &[0.0, 1.0]),
        &[envelope(20_000_000, 180_000_000)],
    )
    .unwrap();
    assert_close(attack.samples[1], 1.0 - (-1.0_f64).exp());

    let release = prepare_transformed_scalar_signal(
        &signal(0, 20_000_000, &[1.0, 0.0]),
        &[envelope(20_000_000, 20_000_000)],
    )
    .unwrap();
    assert_close(release.samples[1], (-1.0_f64).exp());

    let initial = prepare_transformed_scalar_signal(
        &signal(0, 10_000_000, &[0.8, 1.0]),
        &[envelope(100_000_000, 100_000_000)],
    )
    .unwrap();
    assert_eq!(initial.samples[0], 0.8);
}

#[test]
fn envelope_zero_durations_snap_in_their_respective_directions() {
    let zero_attack =
        prepare_transformed_scalar_signal(&signal(0, 10, &[0.0, 1.0]), &[envelope(0, 100)])
            .unwrap();
    assert_eq!(zero_attack.samples, [0.0, 1.0]);

    let zero_release =
        prepare_transformed_scalar_signal(&signal(0, 10, &[1.0, 0.0]), &[envelope(100, 0)])
            .unwrap();
    assert_eq!(zero_release.samples, [1.0, 0.0]);
}

#[test]
fn envelope_is_asymmetric_for_negative_and_positive_values() {
    let transformed = prepare_transformed_scalar_signal(
        &signal(0, 10_000_000, &[-1.0, 1.0, -1.0]),
        &[envelope(10_000_000, 100_000_000)],
    )
    .unwrap();
    let rise = -1.0 + (1.0 - (-1.0_f64).exp()) * 2.0;
    let fall = rise + (1.0 - (-0.1_f64).exp()) * (-1.0 - rise);
    assert_close(transformed.samples[1], rise);
    assert_close(transformed.samples[2], fall);
    assert!(transformed.samples[2] > -1.0);
}

#[test]
fn envelope_time_constant_is_independent_of_the_fixed_hop() {
    let five_ms = prepare_transformed_scalar_signal(
        &signal(0, 5_000_000, &[0.0, 1.0, 1.0, 1.0, 1.0]),
        &[envelope(20_000_000, 180_000_000)],
    )
    .unwrap();
    let ten_ms = prepare_transformed_scalar_signal(
        &signal(0, 10_000_000, &[0.0, 1.0, 1.0]),
        &[envelope(20_000_000, 180_000_000)],
    )
    .unwrap();
    assert_close(five_ms.samples[4], ten_ms.samples[2]);

    let tiny_tau =
        prepare_transformed_scalar_signal(&signal(0, 10_000_000, &[0.0, 1.0]), &[envelope(1, 1)])
            .unwrap();
    assert_close(tiny_tau.samples[1], 1.0);

    let large_tau = prepare_transformed_scalar_signal(
        &signal(0, 10_000_000, &[0.0, 1.0]),
        &[envelope(1_000_000_000_000, 1_000_000_000_000)],
    )
    .unwrap();
    assert!(large_tau.samples[1] > 0.0 && large_tau.samples[1] < 0.000_02);
}

#[test]
fn envelope_runs_at_its_declared_position_in_the_transform_chain() {
    let raw = signal(0, 10_000_000, &[0.0, 2.0]);
    let clamp_then_envelope = prepare_transformed_scalar_signal(
        &raw,
        &[
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
            envelope(10_000_000, 10_000_000),
        ],
    )
    .unwrap();
    let envelope_then_clamp = prepare_transformed_scalar_signal(
        &raw,
        &[
            envelope(10_000_000, 10_000_000),
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
        ],
    )
    .unwrap();
    assert_close(clamp_then_envelope.samples[1], 1.0 - (-1.0_f64).exp());
    assert_eq!(envelope_then_clamp.samples[1], 1.0);
}

#[test]
fn envelope_parameters_and_order_participate_in_signal_identity_and_raw_sharing() {
    use crate::plan::signals::{AudioAnalysisRequirement, CompiledScalarSignals};
    use std::collections::BTreeMap;
    let band = AudioFrequencyBand::new(40.0, 160.0).unwrap();
    let source = RawScalarSignal::Audio(AudioScalarSignal {
        tap: AudioAnalysisTap::Master,
        feature: AudioScalarFeature::BandEnergy(band),
    });
    let same = CompiledScalarSignal::new(source, vec![envelope(20, 180)]);
    let different_attack = CompiledScalarSignal::new(source, vec![envelope(30, 180)]);
    let different_release = CompiledScalarSignal::new(source, vec![envelope(20, 200)]);
    let reordered = CompiledScalarSignal::new(
        source,
        vec![
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).unwrap()),
            envelope(20, 180),
        ],
    );
    let mut interner = ScalarSignalInterner::default();
    assert_eq!(interner.intern(same.clone()), interner.intern(same));
    assert_ne!(
        interner.intern(different_attack),
        interner.intern(different_release)
    );
    assert_ne!(
        interner.intern(reordered),
        interner.intern(CompiledScalarSignal::new(source, vec![envelope(20, 180)]))
    );
    let compiled = CompiledScalarSignals::from_signals(vec![
        CompiledScalarSignal::new(source, vec![envelope(20, 180)]),
        CompiledScalarSignal::new(source, vec![envelope(50, 400)]),
        CompiledScalarSignal::new(
            source,
            vec![CompiledSignalTransform::Gain(
                GainTransform::new(2.0).unwrap(),
            )],
        ),
    ]);
    assert_eq!(compiled.audio_analysis_requirements().iter().len(), 1);
    let prepared = prepare_scalar_signals(
        &compiled,
        BTreeMap::from([(
            AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band)),
            signal(0, 10, &[0.0, 1.0]),
        )]),
    )
    .unwrap();
    assert_eq!(prepared.len(), 3);
}

#[test]
fn silent_master_runs_remap_and_envelope_instead_of_short_circuiting_to_zero() {
    let transformed = prepare_transformed_scalar_signal(
        &signal(0, 10_000_000, &[0.0, 0.0, 0.0]),
        &[
            CompiledSignalTransform::Remap(RemapTransform::new(0.0, 1.0, 1.0, 2.0).unwrap()),
            envelope(20_000_000, 180_000_000),
        ],
    )
    .unwrap();
    assert_eq!(transformed.samples, [1.0, 1.0, 1.0]);
}
