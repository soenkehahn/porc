#[cfg(test)]
pub(crate) mod test {
    pub(crate) fn render_number(n: usize) -> &'static str {
        match n {
            1 => "one",
            2 => "two",
            3 => "three",
            4 => "four",
            n => panic!("TestNode out of range: {}", n),
        }
    }
}
