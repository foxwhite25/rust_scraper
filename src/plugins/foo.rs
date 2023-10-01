use crate::{Handlers, PageType, Scraper};
use log::info;

#[derive(Default)]
pub struct Foo {}

impl Scraper for Foo {
    fn get_handlers(addr: &mut Handlers) {
        addr.on_selector(
            r#"h3 > a"#,
            |ctx, sel| ctx.parse_href(sel),
            |_ctx, crawler, url| async move { crawler.visit(&url, PageType::index()).await },
        )
        .on_selector(
            r#"p.readmore > a"#,
            |ctx, sel| ctx.parse_href(sel),
            |_ctx, crawler, url| async move { crawler.visit(&url, PageType::news()).await },
        )
        .on_selector(
            r#".entry-header > h1"#,
            |_ctx, sel| Some(sel.text().map(|x| x.to_string()).collect::<Vec<_>>()),
            |ctx, _crawler, title| async move {
                let content_type = &ctx.page_type;
                match content_type {
                    PageType::Index => {}
                    PageType::News(_) => {
                        info!("{}", title[0].trim())
                    }
                    PageType::Person(_) => {}
                    PageType::Report(_) => {}
                }
            },
        );
    }

    fn thread_count() -> usize {
        128
    }

    fn starting_address() -> &'static str {
        r#"https://navyrecognition.com/index.php/naval-news.html/"#
    }
}
