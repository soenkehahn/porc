#[cfg(test)]
pub(crate) mod test {
    pub(crate) fn render_number(n: usize) -> &'static str {
        match n {
            1 => "one",
            2 => "two",
            3 => "three",
            4 => "four",
            5 => "five",
            n => panic!("utils::test::render_number: out of range: {}", n),
        }
    }
    pub(crate) fn underline(s: &str) -> String {
        format!("{}\u{35f}", s)
    }
}
