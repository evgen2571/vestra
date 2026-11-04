//! File-backed system font discovery for Python authoring, cached once per process.
use std::{path::PathBuf, sync::OnceLock};

use fontdb::{Database, FaceInfo, Source, Stretch, Style, Weight};
use pyo3::{exceptions::PyValueError, prelude::*};

fn database() -> &'static Database {
    static DATABASE: OnceLock<Database> = OnceLock::new();
    DATABASE.get_or_init(|| {
        let mut database = Database::new();
        database.load_system_fonts();
        database
    })
}

fn path(face: &FaceInfo) -> Option<&PathBuf> {
    match &face.source {
        Source::File(path) | Source::SharedFile(path, _) => Some(path),
        Source::Binary(_) => None,
    }
}

fn families(database: &Database) -> Vec<String> {
    let mut names: Vec<_> = database
        .faces()
        .filter(|face| path(face).is_some())
        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
        .collect();
    names.sort_by_cached_key(|name| (name.to_lowercase(), name.clone()));
    names.dedup_by(|right, left| right.to_lowercase() == left.to_lowercase());
    names
}

fn resolve(database: &Database, family: &str) -> Option<(PathBuf, u32)> {
    let name = family.to_lowercase();
    database
        .faces()
        .filter(|face| {
            face.families
                .iter()
                .any(|(family, _)| family.to_lowercase() == name)
        })
        .filter_map(|face| path(face).map(|path| (face, path)))
        .min_by_key(|(face, path)| {
            (
                face.style != Style::Normal,
                face.stretch != Stretch::Normal,
                face.weight.0.abs_diff(Weight::NORMAL.0),
                (*path).clone(),
                face.index,
            )
        })
        .map(|(face, path)| (path.clone(), face.index))
}

#[pyfunction]
pub(crate) fn list_system_fonts(py: Python<'_>) -> Vec<String> {
    py.detach(|| families(database()))
}

#[pyfunction]
pub(crate) fn _resolve_system_font(py: Python<'_>, family: &str) -> PyResult<(PathBuf, u32)> {
    py.detach(|| resolve(database(), family)).ok_or_else(|| PyValueError::new_err(format!(
        "system font family {family:?} is unavailable; use list_system_fonts(), omit font for the bundled default, or provide a font file path"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Database {
        let mut database = Database::new();
        database
            .load_font_file(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/assets/VestraTest-Regular.ttf"),
            )
            .unwrap();
        database
    }

    #[test]
    fn empty_discovery_never_substitutes() {
        let database = Database::new();
        assert!(families(&database).is_empty());
        assert!(resolve(&database, "Arial").is_none());
    }

    #[test]
    fn exact_case_insensitive_families_are_sorted_and_unique() {
        let fixture = fixture();
        let mut face = fixture.faces().next().unwrap().clone();
        face.families = vec![
            ("Zulu".into(), fontdb::Language::English_UnitedStates),
            ("Ä family".into(), fontdb::Language::English_UnitedStates),
            ("zulu".into(), fontdb::Language::English_UnitedStates),
            ("Alpha".into(), fontdb::Language::English_UnitedStates),
        ];
        let mut database = Database::new();
        database.push_face_info(face);
        assert_eq!(families(&database), ["Alpha", "Zulu", "Ä family"]);
        assert!(resolve(&database, "ä FAMILY").is_some());
        assert!(resolve(&database, "Alph").is_none());
        assert!(resolve(&database, "sans-serif").is_none());
    }

    #[test]
    fn regular_face_and_path_index_ties_are_deterministic() {
        let fixture = fixture();
        let mut face = fixture.faces().next().unwrap().clone();
        let family = face.families[0].0.clone();
        let mut database = Database::new();
        face.source = Source::File("z.ttf".into());
        face.index = 2;
        database.push_face_info(face.clone());
        face.source = Source::File("a.ttc".into());
        face.index = 3;
        database.push_face_info(face.clone());
        face.index = 1;
        database.push_face_info(face.clone());
        face.source = Source::File("0-bold.ttf".into());
        face.weight = Weight::BOLD;
        database.push_face_info(face.clone());
        face.weight = Weight::NORMAL;
        face.style = Style::Italic;
        database.push_face_info(face);
        assert_eq!(resolve(&database, &family), Some(("a.ttc".into(), 1)));
    }
}
