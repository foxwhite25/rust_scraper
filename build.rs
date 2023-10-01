use indoc::formatdoc;
use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::path::PathBuf;

lazy_static! {
    static ref IMPL_REGEX: Regex = Regex::new(r#"impl Scraper for ([\w\d]+) \{"#).unwrap();
}

fn find_struct(path: &PathBuf) -> Option<String> {
    let file_content = fs::read_to_string(path).ok()?;
    let k = IMPL_REGEX.captures(file_content.as_str())?.get(1)?;
    Some(file_content[k.start()..k.end()].to_string())
}

fn main() {
    let plugin_paths = fs::read_dir("./src/plugins")
        .unwrap()
        .filter_map(|file| Some(file.ok()?.path()))
        .filter(|path| !path.ends_with("mod.rs"))
        .collect::<Vec<_>>();

    let plugins = plugin_paths
        .iter()
        .filter_map(|path| {
            let mod_string = path.file_stem()?.to_str()?.to_string();
            Some((
                find_struct(path).unwrap_or_else(|| {
                    panic!("Unable to Find Scraper implimentation in {:?}", path)
                }),
                mod_string,
            ))
        })
        .collect::<Vec<_>>();

    let imports = r#"use crate::{Collector, Scraper};"#.to_string();

    let mods = plugins
        .iter()
        .map(|(name, mod_string)| format!("mod {mod_string};\nuse {mod_string}::{name};"))
        .collect::<Vec<_>>()
        .join("\n");

    let structs = "pub struct Plugins {\n    pub collectors: Vec<Collector>\n}".to_string();

    let collectors = plugins
        .iter()
        .map(|(name, mod_string)| {
            format!(
                "{name}::build_collector(r\"#{mod_string}#\").expect(\"Failed to Build Collector for {mod_string}\")"
            )
        })
        .collect::<Vec<_>>()
        .join(",\n                    ");

    let imps = formatdoc! {"
        impl Plugins {{
            pub fn new() -> Self {{
                let collectors = vec![
                    {}
                ];
                Self {{ collectors }}
            }}
        }}", collectors
    };

    let sum_file = [imports, mods, structs, imps].join("\n") + "\n";

    fs::write("./src/plugins/mod.rs", sum_file).expect("Write Failed");
}
