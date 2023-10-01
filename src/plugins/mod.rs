use crate::{Collector, Scraper};
mod foo;
use foo::Foo;
pub struct Plugins {
    pub collectors: Vec<Collector>
}
impl Plugins {
    pub fn new() -> Self {
        let collectors = vec![
            Foo::build_collector(r"#foo#").expect("Failed to Build Collector for foo")
        ];
        Self { collectors }
    }
}
