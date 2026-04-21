// Excerpt from rustc, adapted.

/// Some unicode characters *have* case, are considered upper case or lower case, but they *can't*
/// be upper cased or lower cased. For the purposes of the lint suggestion, we care about being able
/// to change the char's case.
fn char_has_case(c: char) -> bool {
    !c.to_lowercase().eq(c.to_uppercase())
}

// contains a capitalisable character followed by, or preceded by, an underscore
fn has_underscore_case(s: &str) -> bool {
    let mut last = '\0';
    s.chars()
        .any(|c| match (std::mem::replace(&mut last, c), c) {
            ('_', cs) | (cs, '_') => char_has_case(cs),
            _ => false,
        })
}

pub fn is_camel_case(name: &str) -> bool {
    let name = name.trim_matches('_');
    let Some(first) = name.chars().next() else {
        return true;
    };

    // start with a non-lowercase letter rather than uppercase
    // ones (some scripts don't have a concept of upper/lowercase)
    !(first.is_lowercase() || name.contains("__") || has_underscore_case(name))
}

pub fn to_camel_case(s: &str) -> String {
    s.trim_matches('_')
        .split('_')
        .filter(|component| !component.is_empty())
        .map(|component| {
            let mut camel_cased_component = String::new();

            let mut new_word = true;
            let mut prev_is_lower_case = true;

            for c in component.chars() {
                // Preserve the case if an uppercase letter follows a lowercase letter, so that
                // `camelCase` is converted to `CamelCase`.
                if prev_is_lower_case && c.is_uppercase() {
                    new_word = true;
                }

                if new_word {
                    camel_cased_component.extend(c.to_uppercase());
                } else {
                    camel_cased_component.extend(c.to_lowercase());
                }

                prev_is_lower_case = c.is_lowercase();
                new_word = false;
            }

            camel_cased_component
        })
        .fold(
            (String::new(), None),
            |(acc, prev): (String, Option<String>), next| {
                // separate two components with an underscore if their boundary cannot
                // be distinguished using an uppercase/lowercase case distinction
                let join = if let Some(prev) = prev {
                    let l = prev.chars().last().unwrap();
                    let f = next.chars().next().unwrap();
                    !char_has_case(l) && !char_has_case(f)
                } else {
                    false
                };
                (acc + if join { "_" } else { "" } + &next, Some(next))
            },
        )
        .0
}

pub fn to_snake_case(mut name: &str) -> String {
    let mut words = vec![];
    // Preserve leading underscores
    name = name.trim_start_matches(|c: char| {
        if c == '_' {
            words.push(String::new());
            true
        } else {
            false
        }
    });
    for s in name.split('_') {
        let mut last_upper = false;
        let mut buf = String::new();
        if s.is_empty() {
            continue;
        }
        for ch in s.chars() {
            if !buf.is_empty() && buf != "'" && ch.is_uppercase() && !last_upper {
                words.push(buf);
                buf = String::new();
            }
            last_upper = ch.is_uppercase();
            buf.extend(ch.to_lowercase());
        }
        words.push(buf);
    }
    words.join("_")
}

pub fn is_snake_case(ident: &str) -> bool {
    if ident.is_empty() {
        return true;
    }
    let ident = ident.trim_start_matches('\'');
    let ident = ident.trim_matches('_');

    if ident.contains("__") {
        return false;
    }

    // This correctly handles letters in languages with and without
    // cases, as well as numbers and underscores.
    !ident.chars().any(char::is_uppercase)
}

pub fn is_upper_case(name: &str) -> bool {
    !name.chars().any(|c| c.is_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_underscore() {
        let ident = "_";
        assert!(is_snake_case(ident));
        assert_eq!(to_camel_case(ident), "");
    }

    #[test]
    fn crate_name_with_disambiguator() {
        let ident = "foo_bar#1";
        assert!(is_snake_case(ident));
        assert_eq!(to_camel_case(ident), "FooBar#1");
    }
}
