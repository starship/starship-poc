pub struct BenchConfig {
    pub name: &'static str,
    pub source: &'static str,
}
impl std::fmt::Display for BenchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
