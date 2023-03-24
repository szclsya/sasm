use super::{PkgVersion, PkgVersionSegment};

use anyhow::{Result, bail};
use nom::{
    character::{complete::*, is_alphanumeric},
    error::{context, ErrorKind, ParseError},
    sequence::*,
    IResult, InputTakeAtPosition,
    combinator::eof
};


pub fn parse_version(i: &str) -> IResult<&str, PkgVersion> {
    let (tmp_i, epoch) = match context(
        "Parsing epoch...",
        pair::<_, _, _, nom::error::Error<&str>, _, _>(digit1, char(':')),
    )(i)
    {
        Ok((i, (epoch, _))) => (i, epoch.parse().unwrap()),
        Err(_) => (i, 0),
    };

    // Try with or without epoch
    let (i, (upstream_version, revision)) = match upstream_version(tmp_i) {
        Ok(x) => x,
        Err(_) => upstream_version(i)?,
    };

    let res = PkgVersion {
        epoch,
        version: upstream_version,
        revision,
    };

    Ok((i, res))
}

impl TryFrom<&str> for PkgVersion {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self> {
        match parse_version(s) {
            Ok((_, ver)) => Ok(ver),
            Err(e) => bail!("Error parsing package version: {e}")
        }
    }
}

fn is_upstream_version_char(c: char) -> bool {
    c.is_alphanumeric() || is_upstream_version_separater(c)
}

fn is_upstream_version_separater(c: char) -> bool {
    c == '.' || c == '-' || c == '_' || c == '~' || c == '+'
}

fn upstream_version_separater(i: &str) -> IResult<&str, &str> {
    i.split_at_position1_complete(|item| !is_upstream_version_separater(item), ErrorKind::Char)
}

fn revision(i: &str) -> IResult<&str, &str> {
    let (i, _) = char('-')(i)?;
    let (i, rev) = digit1(i)?;
    let (i, _) = eof(i)?;
    Ok((i, rev))
}

fn upstream_version(i: &str) -> IResult<&str, (Vec<PkgVersionSegment>, Option<u64>)> {
    if i.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::from_error_kind(i, ErrorKind::Eof)));
    }

    if !i.starts_with(|c: char| c.is_alphanumeric()) {
        return Err(nom::Err::Error(nom::error::Error::from_error_kind(i, ErrorKind::Char)));
    }

    let mut result = Vec::new();
    let mut rev = None;
    let mut ti = i;
    loop {
       if ti.len() == 0 {
            // Our job is done here
            break;
       } else if let Ok((i, r)) = revision(ti) {
           // We've reached the end and there's a revision
           rev = Some(r.parse().unwrap());
           ti = i;
           break;
       } else if let Ok((i, digits)) = digit1::<_, ()>(ti) {
           // We got a digit segment!
           result.push(PkgVersionSegment::Number(digits.parse().unwrap()));
           ti = i;
        } else if let Ok((i, chars)) = alpha1::<_, ()>(ti) {
            // We got a character segment!
            result.push(PkgVersionSegment::Alphabetic(chars.to_owned()));
            ti = i;
        } else if let Ok((i, chars)) = upstream_version_separater(ti) {
            // Some characters we don't care
            result.push(PkgVersionSegment::Separater(chars.to_owned()));
            ti = i;
        } else {
           // We've reached something we don't know about. Stop parsing
           break;
        }
    }

    Ok((ti, (result, rev)))
}
