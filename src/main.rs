#![feature(tuple_trait)]

use idfk::Plugins;
use log::LevelFilter::Info;

#[tokio::main]
async fn main() {
    env_logger::Builder::new().filter_level(Info).init();
    let p = Plugins::new();
    p.collectors[0].start().await;
    // let base = Url::parse("https://google.com").unwrap();
    // println!("{:?}", base.join("https://youtube.com").unwrap());
}
