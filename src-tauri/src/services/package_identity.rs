use std::path::Path;

/// Decompose a filename without inventing an alias for a renamed package.
/// Only a complete, in-range decimal version can be used for version ordering.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PackageIdentity<'a> {
    pub id: &'a str,
    pub creator: &'a str,
    pub name: &'a str,
    pub version: Option<i32>,
}

pub(crate) fn from_path(path: &Path) -> Option<PackageIdentity<'_>> {
    if !path.extension()?.eq_ignore_ascii_case("var") {
        return None;
    }
    from_id(path.file_stem()?.to_str()?)
}

pub(crate) fn from_id(id: &str) -> Option<PackageIdentity<'_>> {
    if id.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || id.split('.').any(|part| part.trim().is_empty() || part.trim() != part)
    {
        return None;
    }
    // The first separator identifies the author; only the last identifies a
    // version. Dots in the package name and the original ID remain untouched.
    let (creator, remainder) = id.split_once('.')?;
    let (name, version) = match remainder.rsplit_once('.') {
        Some((name, text)) => {
            let version = text.bytes().all(|c| c.is_ascii_digit())
                .then(|| text.parse::<i32>().ok()).flatten();
            (name, version)
        }
        None => (remainder, None),
    };
    Some(PackageIdentity { id, creator, name, version })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposes_names_without_guessing_versions_or_changing_ids() {
        for (filename, creator, name, version) in [
            ("Author.Sub.Name.12.VAR", "Author", "Sub.Name", Some(12)),
            ("Author.Name.001.var", "Author", "Name", Some(1)),
            ("作者.长发.0.var", "作者", "长发", Some(0)),
            ("Qing.Alet01.1_2.var", "Qing", "Alet01", None),
            ("Author.Name.1 (2).var", "Author", "Name", None),
            ("Author.Name.v3.var", "Author", "Name", None),
            ("V2.Morphs.latest.var", "V2", "Morphs", None),
            ("Author.Name.var", "Author", "Name", None),
            ("Author.Name.2147483648.var", "Author", "Name", None),
            ("Author.Name.-1.var", "Author", "Name", None),
        ] {
            let parsed = from_path(Path::new(filename)).unwrap();
            assert_eq!(parsed.id, &filename[..filename.len() - 4]);
            assert_eq!((parsed.creator, parsed.name, parsed.version), (creator, name, version));
        }
    }

    #[test]
    fn ambiguous_or_unsafe_names_are_not_silently_repaired() {
        for filename in ["BadName.var", ".Name.1.var", "Author..1.var",
            "Author.Name..var", "Author. .1.var", " Author.Name.1.var",
            "Author.Name.1 .var", "Author.Na:me.1.var", "Author.Name.1.zip"] {
            assert!(from_path(Path::new(filename)).is_none(), "{filename}");
        }
    }
}
