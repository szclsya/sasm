/// Parse Contents files
use anyhow::{ Result, bail };
use nom::{
    bytes::complete::tag,
    bytes::complete::take_until1,
    character::complete::{space0, space1},
    combinator::eof,
    error::ErrorKind,
    multi::separated_list1,
    IResult, InputTakeAtPosition,
};

pub fn parse_contents_line(i: &str) -> Result<(&str, Vec<( &str, &str )>)> {
    let (_, (path, packages)) = match contents_line(i) {
        Ok(res) => res,
        Err(e) => bail!("Invalid Contents line: {}", e),
    };
    Ok((path, packages))
}

pub fn contents_line(i: &str) -> IResult<&str, (&str, Vec<(&str, &str)>)> {
    let (i, mut path) = take_until1(" ")(i)?;
    let (i, _) = space1(i)?;
    let (i, packages) = separated_list1(package_separator, package)(i)?;
    let (i, _) = eof(i)?;

    // Normalize path
    if path.starts_with("./") {
        path = &path[2..];
    }

    Ok((i, (path, packages)))
}

fn package_separator(i: &str) -> IResult<&str, ()> {
    let (i, _) = tag(",")(i)?;
    let (i, _) = space0(i)?;
    Ok((i, ()))
}

fn package(i: &str) -> IResult<&str, (&str, &str)> {
    let (i, section) = take_until1("/")(i)?;
    let (i, _) = tag("/")(i)?;
    let (i, name) = package_name(i)?;
    Ok((i, (section, name)))
}

fn is_pkgname_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '.' || c == '+'
}

fn is_pkgname_with_var_char(c: char) -> bool {
    is_pkgname_char(c) || c == '{' || c == '_' || c == '}'
}

fn package_name(i: &str) -> IResult<&str, &str> {
    i.split_at_position1_complete(|item| !is_pkgname_with_var_char(item), ErrorKind::Char)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_contents_line() {
        let tests = vec![
            (
                "simple/path sec/pkg1",
                ("simple/path", vec![("sec", "pkg1")]),
            ),
            (
                "simple/path   sec/pkg1,sec2/pkg2",
                ("simple/path", vec![("sec", "pkg1"), ("sec2", "pkg2")]),
            ),
            (
                "./bad/path   sec/pkg1,sec2/pkg2",
                ("bad/path", vec![("sec", "pkg1"), ("sec2", "pkg2")]),
            ),
        ];
        for (t, r) in tests {
            assert_eq!(contents_line(t).unwrap(), ("", r));
        }
    }
}
