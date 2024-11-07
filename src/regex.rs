use crate::R;

#[derive(Debug)]
pub(crate) enum Regex {
    Regex { regex: regex::Regex },
    Invalid { regex: String },
}

impl Regex {
    pub(crate) fn empty() -> R<Regex> {
        Regex::new("")
    }

    pub(crate) fn new(regex: &str) -> R<Regex> {
        Ok(Regex::Regex {
            regex: regex::Regex::new(regex)?,
        })
    }

    pub(crate) fn is_match(&self, s: &str) -> bool {
        match self {
            Regex::Regex { regex } => regex.is_match(s),
            Regex::Invalid { .. } => false,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Regex::Regex { regex } => regex.as_str(),
            Regex::Invalid { regex } => regex.as_str(),
        }
    }

    pub(crate) fn modify<F>(&mut self, f: F)
    where
        F: FnOnce(&mut String),
    {
        let mut regex: String = self.as_str().to_string();
        f(&mut regex);
        *self = match regex::Regex::new(&regex) {
            Ok(regex) => Regex::Regex { regex },
            Err(_) => Regex::Invalid { regex },
        }
    }
}
