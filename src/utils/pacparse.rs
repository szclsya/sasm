/// Parse pacman style package database files
use nom::{
    bytes::complete::{take, take_till, take_until1},
    character::complete::{alphanumeric1, anychar, char},
    IResult,
};

/// Parse the key part of a paragraph, like `%NAME%`
fn parse_key(i: &str) -> IResult<&str, &str> {
    let (i, _) = char('%')(i)?;
    let (i, key) = alphanumeric1(i)?;
    let (i, _) = char('%')(i)?;
    // There should be a newline after the key line
    let (i, _) = char('\n')(i)?;

    Ok((i, key))
}

/// Parse the value part of a paragraph that ends with an empty line
fn parse_value_with_empty_line(i: &str) -> IResult<&str, &str> {
    // It should be multiple lines until an empty line
    let (i, content) = take_until1("\n\n")(i)?;
    // Eat the new lines
    let (i, _) = char('\n')(i)?;
    let (i, _) = char('\n')(i)?;

    Ok((i, content))
}

/// Parse the value part of a paragraph that ends with EOF
fn parse_value(mut i: &str) -> IResult<&str, Vec<String>> {
    let mut lines = Vec::new();
    loop {
        eprint!("{i}bruh");
        let (x, content) = take_till(|c| c == '\n')(i)?;
        let (x, _) = char('\n')(x)?;
        i = x;
        if content.is_empty() {
            break;
        }
        lines.push(content.to_owned());
        if x.is_empty() {
            break;
        }
    }
    Ok((i, lines))
}

/// Parse a key-value pair in pacman's package description syntax
fn parse_pair(i: &str) -> IResult<&str, (String, Vec<String>)> {
    let (i, key) = parse_key(i)?;
    let (i, lines) = parse_value(i)?;

    Ok((i, (key.to_owned(), lines)))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn try_parse() {
        assert_eq!(("", "BRUH"), parse_key("%BRUH%\n").unwrap());
        assert_eq!(
            (
                "something else",
                (
                    "NAME".to_string(),
                    vec![
                        "A multiple".to_string(),
                        "line".to_string(),
                        "paragraph.".to_string()
                    ]
                )
            ),
            parse_pair(
                "%NAME%
A multiple
line
paragraph.

something else"
            )
            .unwrap()
        );
        assert_eq!(
            (
                "",
                (
                    "NAME".to_string(),
                    vec![
                        "A multiple".to_string(),
                        "line".to_string(),
                        "paragraph.".to_string()
                    ],
                )
            ),
            parse_pair(
                "%NAME%
A multiple
line
paragraph.
"
            )
            .unwrap()
        );
    }
}
