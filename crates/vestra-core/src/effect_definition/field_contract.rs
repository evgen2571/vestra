//! Closed field classification for the catalog's scalar/plain-track associations.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Scalar,
    PlainTrack,
    Other,
    Timing,
    Point,
}

pub(crate) trait CatalogField {
    const KIND: FieldKind;
}

pub(crate) const fn association_matches(
    field: &str,
    kind: FieldKind,
    associations: &[(&str, FieldKind)],
) -> bool {
    let mut count = 0;
    let mut index = 0;
    while index < associations.len() {
        let (name, associated_kind) = associations[index];
        if same_name(field.as_bytes(), name.as_bytes()) {
            if kind as u8 != associated_kind as u8 {
                return false;
            }
            count += 1;
        }
        index += 1;
    }
    count
        == if matches!(kind, FieldKind::Scalar | FieldKind::PlainTrack) {
            1
        } else {
            0
        }
}

const fn same_name(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

macro_rules! check_fields {
    ($kind:ident, $fields:tt, [$($scalar:ident),*], [$($track:ident),*]) => {
        $crate::effect_definition::field_contract::check_fields!(@associations $kind, $fields, [$($scalar),*], [$($track),*]);
        $crate::effect_definition::field_contract::check_fields!(@projections $kind; $fields);
    };
    (@associations $kind:ident, { $($(#[$attribute:meta])* $name:ident: $ty:ty,)* },
     [$($scalar:ident),*], [$($track:ident),*]) => {
        const _: () = {
            use $crate::effect_definition::field_contract::{CatalogField, FieldKind, association_matches};
            let associations = &[
                $((stringify!($scalar), FieldKind::Scalar),)*
                $((stringify!($track), FieldKind::PlainTrack),)*
            ];
            $(assert!(association_matches(stringify!($name), <$ty as CatalogField>::KIND, associations),
                concat!(stringify!($kind), ".", stringify!($name), ": incomplete or duplicate scalar/plain-track association"));)*
            let expected = 0 $(+ (matches!(<$ty as CatalogField>::KIND, FieldKind::Scalar | FieldKind::PlainTrack)) as usize)*;
            assert!(associations.len() == expected, concat!(stringify!($kind), ": association names a nonexistent field"));
        };
    };
    (@projections $kind:ident; { $(#[$attribute:meta])* $name:ident: ActiveInterval, $($rest:tt)* }) => {
        $crate::effect_definition::field_contract::check_fields!(@projections $kind; { $($rest)* });
    };
    (@projections $kind:ident; { $(#[$attribute:meta])* $name:ident: crate::project::ActiveInterval, $($rest:tt)* }) => {
        $crate::effect_definition::field_contract::check_fields!(@projections $kind; { $($rest)* });
    };
    (@projections $kind:ident; { $(#[$attribute:meta])* $name:ident: CompiledPointProperty, $($rest:tt)* }) => {
        $crate::effect_definition::field_contract::check_fields!(@projections $kind; { $($rest)* });
    };
    (@projections $kind:ident; { $(#[$attribute:meta])* $name:ident: crate::plan::CompiledPointProperty, $($rest:tt)* }) => {
        $crate::effect_definition::field_contract::check_fields!(@projections $kind; { $($rest)* });
    };
    (@projections $kind:ident; { $(#[$attribute:meta])* $name:ident: $ty:ty, $($rest:tt)* }) => {
        const _: () = {
            use $crate::effect_definition::field_contract::{CatalogField, FieldKind};
            assert!(!matches!(<$ty as CatalogField>::KIND, FieldKind::Timing | FieldKind::Point),
                concat!(stringify!($kind), ".", stringify!($name), ": use canonical point/timing type paths"));
        };
        $crate::effect_definition::field_contract::check_fields!(@projections $kind; { $($rest)* });
    };
    (@projections $kind:ident; {}) => {};
}

pub(crate) use check_fields;

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    // Compile the actual declaration checker, without building another Cargo workspace.
    fn compile(fields: &str, scalars: &str, tracks: &str) -> std::process::Output {
        let directory = std::env::temp_dir().join(format!(
            "vestra-field-contract-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let source = format!(
            r#"
            #![allow(dead_code, unused_imports)]
            mod effect_definition {{
                #[path = {:?}] pub(crate) mod field_contract;
            }}
            use effect_definition::field_contract::{{CatalogField, FieldKind, check_fields}};
            struct Scalar;
            struct Track;
            struct PointTrack;
            impl CatalogField for Scalar {{ const KIND: FieldKind = FieldKind::Scalar; }}
            impl CatalogField for Track {{ const KIND: FieldKind = FieldKind::PlainTrack; }}
            impl CatalogField for PointTrack {{ const KIND: FieldKind = FieldKind::Other; }}
            struct ActiveInterval;
            impl CatalogField for ActiveInterval {{ const KIND: FieldKind = FieldKind::Timing; }}
            struct CompiledPointProperty;
            impl CatalogField for CompiledPointProperty {{ const KIND: FieldKind = FieldKind::Point; }}
            type PointAlias = CompiledPointProperty;
            type TimingAlias = ActiveInterval;
            mod project {{ pub(crate) use crate::ActiveInterval; }}
            mod plan {{ pub(crate) use crate::CompiledPointProperty; }}
            type ScalarAlias = Scalar;
            check_fields!(Fixture, {{ {fields} }}, [{scalars}], [{tracks}]);
        "#,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/effect_definition/field_contract.rs"
            )
        );
        let path = directory.join("fixture.rs");
        fs::write(&path, source).unwrap();
        let result = Command::new("rustc")
            .args(["--edition=2024", "--crate-type=lib", "--emit=metadata"])
            .arg(&path)
            .arg("--out-dir")
            .arg(&directory)
            .output()
            .unwrap();
        fs::remove_dir_all(directory).unwrap();
        result
    }

    #[test]
    fn incomplete_or_duplicate_associations_fail_compilation() {
        for (fields, scalars, tracks, field) in [
            ("amount: Scalar,", "", "", "amount"),
            ("softness: Track,", "", "", "softness"),
            ("amount: Scalar,", "", "amount", "amount"),
            (
                "amount: Scalar, phase: Scalar,",
                "amount, amount",
                "",
                "amount",
            ),
            ("point: PointTrack,", "point", "", "point"),
        ] {
            let output = compile(fields, scalars, tracks);
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                !output.status.success(),
                "unaccounted field compiled: {fields}"
            );
            assert!(
                error.contains(&format!("Fixture.{field}: incomplete or duplicate")),
                "{error}"
            );
        }
    }

    #[test]
    fn qualified_attributed_aliases_and_specialized_tracks_are_supported() {
        let output = compile(
            "#[serde(default)] amount: crate::ScalarAlias, phase: Scalar, point: PointTrack, softness: crate::Track,",
            "amount, phase",
            "softness",
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn nonexistent_associations_fail_compilation() {
        let output = compile("amount: Scalar,", "amount, missing", "");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("Fixture: association names a nonexistent field")
        );
    }

    #[test]
    fn unknown_field_types_require_explicit_classification() {
        let output = compile("unknown: u32,", "", "");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("u32: CatalogField"));
    }

    #[test]
    fn specialized_aliases_cannot_silently_skip_token_based_projections() {
        for fields in ["timing: crate::TimingAlias,", "center: PointAlias,"] {
            let output = compile(fields, "", "");
            assert!(
                !output.status.success(),
                "specialized alias silently compiled: {fields}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("use canonical point/timing type paths")
            );
        }
        let output = compile(
            "#[serde(flatten)] timing: crate::project::ActiveInterval, center: crate::plan::CompiledPointProperty,",
            "",
            "",
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
