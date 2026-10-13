use rusqlite::Connection;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct InstalledPackage {
    pub id: String,
    pub creator: String,
    pub name: String,
    pub version: i32,
}

/// Built once per snapshot. Keys fold ASCII only, matching eq_ignore_ascii_case.
pub(crate) struct DependencyIndex<'a> {
    exact: HashMap<String, &'a InstalledPackage>,
    best: HashMap<(String, String), &'a InstalledPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyResolution<'a> {
    pub resolved: Option<&'a str>,
    pub best: Option<&'a str>,
    pub installed_version: Option<i32>,
    pub status: &'static str,
}

impl<'a> DependencyIndex<'a> {
    pub fn new(packages: &'a [InstalledPackage]) -> Self {
        let mut exact = HashMap::with_capacity(packages.len());
        let mut best: HashMap<_, &InstalledPackage> = HashMap::with_capacity(packages.len());
        for pkg in packages {
            // Readable nonstandard names are not evidence of an installed game
            // version. In particular, a literal .latest must not satisfy itself.
            if !has_numeric_identity(pkg) { continue; }
            // find() keeps the first exact hit; max_by_key() keeps the last tie.
            exact.entry(pkg.id.to_ascii_lowercase()).or_insert(pkg);
            let key = (pkg.creator.to_ascii_lowercase(), pkg.name.to_ascii_lowercase());
            if best.get(&key).map_or(true, |previous| pkg.version >= previous.version) {
                best.insert(key, pkg);
            }
        }
        Self { exact, best }
    }

    pub fn resolve(&self, id: &str) -> DependencyResolution<'a> {
        let exact = self.exact.get(&id.to_ascii_lowercase());
        let parsed = parse_dependency_parts(id);
        let best = parsed.as_ref().and_then(|(creator, name, _)| {
            self.best.get(&(creator.to_ascii_lowercase(), name.to_ascii_lowercase()))
        });
        let required = parsed.as_ref().and_then(|(_, _, version)| *version);
        let resolved = exact.copied().or_else(|| best.copied().filter(|pkg| {
            required.map_or(true, |version| pkg.version >= version)
        }));
        DependencyResolution {
            resolved: resolved.map(|pkg| pkg.id.as_str()),
            best: exact.or(best).map(|pkg| pkg.id.as_str()),
            installed_version: best.map(|pkg| pkg.version),
            status: if resolved.is_some() { "satisfied" }
                else if best.is_some() { "lower_version" }
                else { "missing" },
        }
    }
}

fn has_numeric_identity(pkg: &InstalledPackage) -> bool {
    super::package_identity::from_id(&pkg.id)
        .is_some_and(|id| id.version == Some(pkg.version))
}

pub(crate) fn load_installed_packages(
    conn: &Connection,
) -> rusqlite::Result<Vec<InstalledPackage>> {
    let mut stmt = conn.prepare("SELECT id, creator, name, version FROM packages")?;
    let rows = stmt.query_map([], |row| {
        Ok(InstalledPackage {
            id: row.get(0)?,
            creator: row.get(1)?,
            name: row.get(2)?,
            version: row.get(3)?,
        })
    })?;
    rows.collect()
}

pub(crate) fn parse_dependency_parts(id: &str) -> Option<(String, String, Option<i32>)> {
    let (creator, remainder) = id.split_once('.')?;
    let (name, version) = match remainder.rsplit_once('.') {
        // Keep the existing permissive behavior: latest and nonnumeric suffixes
        // impose no minimum version; dots inside the package name are preserved.
        Some((name, version)) => (name, version.parse().ok()),
        None => (remainder, None),
    };
    Some((creator.to_string(), name.to_string(), version))
}

#[cfg(test)]
pub(crate) fn resolve_installed_dependency_id(
    depends_on_id: &str,
    installed_packages: &[InstalledPackage],
) -> Option<String> {
    // Exact IDs take precedence even when a newer version is available.
    if let Some(pkg) = installed_packages
        .iter()
        .find(|pkg| has_numeric_identity(pkg) && pkg.id.eq_ignore_ascii_case(depends_on_id))
    {
        return Some(pkg.id.clone());
    }

    let (creator, name, required_version) = parse_dependency_parts(depends_on_id)?;
    installed_packages
        .iter()
        .filter(|pkg| {
            has_numeric_identity(pkg) && pkg.creator.eq_ignore_ascii_case(&creator)
                && pkg.name.eq_ignore_ascii_case(&name)
                && required_version.map_or(true, |required| pkg.version >= required)
        })
        .max_by_key(|pkg| pkg.version)
        .map(|pkg| pkg.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dotted_names_and_preserves_legacy_version_rules() {
        for (id, expected) in [
            ("Author.Package", Some(("Author", "Package", None))),
            (
                "Author.Sub.Package.12",
                Some(("Author", "Sub.Package", Some(12))),
            ),
            ("Author.Package.LaTeSt", Some(("Author", "Package", None))),
            ("Author.Package.invalid", Some(("Author", "Package", None))),
            (
                "Author.Package.2147483648",
                Some(("Author", "Package", None)),
            ),
            ("Author.Package.-1", Some(("Author", "Package", Some(-1)))),
            ("Author.Package.", Some(("Author", "Package", None))),
            (".", Some(("", "", None))),
            ("", None),
            ("Author", None),
        ] {
            assert_eq!(
                parse_dependency_parts(id),
                expected.map(|(creator, name, version)| (creator.into(), name.into(), version)),
                "{id}",
            );
        }
    }

    fn installed() -> Vec<InstalledPackage> {
        [1, 10, 2]
            .into_iter()
            .map(|version| InstalledPackage {
                id: format!("Author.Sub.Package.{version}"),
                creator: "Author".into(),
                name: "Sub.Package".into(),
                version,
            })
            .collect()
    }

    #[test]
    fn exact_dependency_keeps_its_version_and_installed_spelling() {
        assert_eq!(
            resolve_installed_dependency_id("author.sub.package.1", &installed()).as_deref(),
            Some("Author.Sub.Package.1"),
        );
    }

    #[test]
    fn fallback_chooses_highest_numeric_version_that_meets_requirement() {
        for (id, expected) in [
            ("author.sub.package.latest", Some("Author.Sub.Package.10")),
            ("Author.Sub.Package.3", Some("Author.Sub.Package.10")),
            ("Author.Sub.Package.invalid", Some("Author.Sub.Package.10")),
            ("Author.Sub.Package.11", None),
            ("Other.Sub.Package.latest", None),
            ("Author.Other.latest", None),
            ("invalid", None),
        ] {
            assert_eq!(
                resolve_installed_dependency_id(id, &installed()).as_deref(),
                expected,
                "{id}"
            );
        }
        assert_eq!(
            resolve_installed_dependency_id("Author.Package.1", &[]),
            None
        );
    }

    #[test]
    fn versionless_dependency_and_ties_keep_existing_selection_order() {
        let mut packages = installed();
        for pkg in &mut packages {
            pkg.name = "Package".into();
            pkg.id = format!("Author.Package.{}", pkg.version);
        }
        let mut duplicate = packages[1].clone();
        duplicate.id = "author.package.10".into();
        packages.push(duplicate);
        assert_eq!(
            resolve_installed_dependency_id("Author.Package", &packages).as_deref(),
            Some("author.package.10")
        );
        assert_eq!(
            resolve_installed_dependency_id("Author.Package.10", &packages).as_deref(),
            Some("Author.Package.10")
        );
    }

    #[test]
    fn recovered_filenames_never_supply_a_numeric_or_latest_dependency() {
        let packages: Vec<_> = ["1_2", "latest", "v3", "2147483648"].into_iter()
            .map(|suffix| InstalledPackage { id: format!("Author.Package.{suffix}"),
                creator: "Author".into(), name: "Package".into(), version: 0 }).collect();
        let index = DependencyIndex::new(&packages);
        for query in ["Author.Package", "Author.Package.0", "Author.Package.1", "Author.Package.latest"] {
            assert_eq!(index.resolve(query).resolved, None, "{query}");
            assert_eq!(resolve_installed_dependency_id(query, &packages), None, "{query}");
        }
    }

    #[test]
    fn database_loader_preserves_values_and_reports_bad_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE packages (id TEXT, creator TEXT, name TEXT, version INTEGER);
            INSERT INTO packages VALUES ('Author.Sub.Package.2', 'Author', 'Sub.Package', 2);",
        )
        .unwrap();
        let packages = load_installed_packages(&conn).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(
            resolve_installed_dependency_id("author.sub.package.latest", &packages).as_deref(),
            Some("Author.Sub.Package.2")
        );
        conn.execute("INSERT INTO packages VALUES (NULL, 'Author', 'Bad', 1)", [])
            .unwrap();
        assert!(load_installed_packages(&conn).is_err());
    }
}
