use crate::R;

#[derive(Debug)]
pub(crate) enum Regex {
    Regex { regex: regex::Regex },
    Invalid { regex: String },
}

impl Regex {
    pub(crate) fn empty() -> R<Regex> {
        Ok(Regex::new(::regex::Regex::new("")?))
    }

    pub(crate) fn new(regex: ::regex::Regex) -> Regex {
        Regex::Regex { regex }
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

    pub(crate) fn modify(&mut self, f: impl FnOnce(&mut String)) {
        let mut regex: String = self.as_str().to_string();
        f(&mut regex);
        *self = match regex::Regex::new(&regex) {
            Ok(regex) => Regex::Regex { regex },
            Err(_) => Regex::Invalid { regex },
        }
    }
}
