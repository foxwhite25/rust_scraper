#![feature(tuple_trait)]

use idfk::{Handlers, NewsData, PageType, Scraper};
use log::LevelFilter::Info;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use log::info;
use url::Url;

#[derive(Default)]
struct Foo {
    foo: AtomicUsize,
}

impl Scraper for Foo {
    fn get_handlers(addr: &mut Handlers<Self>) {
        addr.on_selector(
            r#"h3 > a"#,
            |ctx, sel| ctx.current_address.join(sel.value().attr("href")?).ok(),
            |_ctx, crawler, url| async move { crawler.visit(url, PageType::Index).await },
        )
        .on_selector(
            r#"p.readmore > a"#,
            |ctx, sel| ctx.current_address.join(sel.value().attr("href")?).ok(),
            |_ctx, crawler, url| async move {
                crawler
                    .visit(url, PageType::News(Box::from(NewsData {})))
                    .await
            },
        )
        .on_selector(
            r#"h1"#,
            |_ctx, sel| Some(sel.text().collect::<String>()),
            |ctx, _crawler, title| async move {
                let content_type = &ctx.page_type;
                match content_type {
                    PageType::Index => {}
                    PageType::News(_) => {
                        info!("{}", title.trim())
                    }
                    PageType::Person(_) => {}
                    PageType::Report(_) => {}
                }
            },
        )
        .on_response(
            |_ctx, resp| Some(resp.url.clone()),
            |ctx, _crawler, url| async move {
                info!("{}, {}", url, ctx.state.foo.fetch_add(1, Relaxed))
            },
        );
    }

    fn thread_count() -> usize {
        128
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::new().filter_level(Info).init();
    let start = Url::parse(r#"https://navyrecognition.com/index.php/naval-news.html/"#).unwrap();
    Foo::build_collector().unwrap().start(start).await;
    // let base = Url::parse("https://google.com").unwrap();
    // println!("{:?}", base.join("https://youtube.com").unwrap());
}
