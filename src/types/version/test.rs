#[cfg(test)]
mod test {
    use super::super::PkgVersion;
    use std::cmp::Ordering::*;
    #[test]
    fn pkg_ver_ord() {
        let source = vec![
            ("1.1.1", Less, "1.1.2"),
            ("1b", Greater, "1a"),
            ("1", Less, "1.1"),
            ("1.0", Less, "1.1"),
            ("1.2", Less, "1.11"),
            ("1.0-1", Less, "1.1"),
            ("1.0-1", Less, "1.0-12"),
            // make them different for sorting
            ("1:1.0-0", Equal, "1:1.0"),
            ("1.0", Equal, "1.0"),
            ("1.0-1", Equal, "1.0-1"),
            ("1:1.0-1", Equal, "1:1.0-1"),
            ("1:1.0", Equal, "1:1.0"),
            ("1.0-1", Less, "1.0-2"),
            //("1.0final-5sarge1", Greater, "1.0final-5"),
            ("1.0final-5", Greater, "1.0a7-2"),
            ("0.9.2-5", Less, "0.9.2+cvs.1.0.dev.2004.07.28-1"),
            ("1:500", Less, "1:5000"),
            ("100:500", Greater, "11:5000"),
            ("1.0.4-2", Greater, "1.0pre7-2"),
            ("1.5rc1", Less, "1.5"),
            ("1.5rc1", Less, "1.5+1"),
            ("1.5rc1", Less, "1.5rc2"),
            ("1.5rc1", Greater, "1.5dev0"),
        ];

        for e in source {
            println!("Comparing {} vs {}", e.0, e.2);
            println!(
                "{:#?} vs {:#?}",
                PkgVersion::try_from(e.0).unwrap(),
                PkgVersion::try_from(e.2).unwrap()
            );
            assert_eq!(
                PkgVersion::try_from(e.0).unwrap().cmp(&PkgVersion::try_from(e.2).unwrap()),
                e.1
            );
        }
    }

    #[test]
    fn pkg_ver_eq() {
        let source = vec![("1.1+git2021", "1.1+git2021")];
        for e in &source {
            assert_eq!(PkgVersion::try_from(e.0).unwrap(), PkgVersion::try_from(e.1).unwrap());
        }
    }
}
