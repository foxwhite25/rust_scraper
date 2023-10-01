use crate::{Context, PageType};
use crate::{Crawler, Respond};
use log::debug;
use reqwest::Client;
use scraper::{ElementRef, Selector};
use std::fmt::Display;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use url::Url;

type SelectorHandler = Box<dyn Fn(Arc<Context>, Arc<Crawler>, ElementRef) + Sync + Send>;
type ResponseHandler = Box<dyn Fn(Arc<Context>, Arc<Crawler>, &Respond) + Sync + Send>;
pub type SelectorHandlerPairs = Vec<(Selector, SelectorHandler)>;
pub type ResponseHandlers = Vec<ResponseHandler>;

pub struct Collector {
    crawler: Arc<Crawler>,
    running: Arc<AtomicBool>,
    starting_address: Url,
    name: &'static str,
}

impl Collector {
    pub async fn start(&self) -> Option<()> {
        debug!("Starting {}", self.name);
        let crawler = self.crawler.clone();
        let sem = crawler.semaphore.clone();
        let thread = self.crawler.max_permit;

        crawler.visit(&self.starting_address, PageType::Index).await;
        self.running.store(true, Ordering::Relaxed);
        loop {
            sleep(Duration::from_millis(20)).await;
            if sem.available_permits() == thread {
                break;
            }
        }
        self.running.store(false, Ordering::Relaxed);
        Some(())
    }
}

#[derive(Default)]
pub struct Handlers {
    pub(crate) selector_handler: SelectorHandlerPairs,
    pub(crate) response_handler: ResponseHandlers,
}

impl Handlers {
    pub fn new() -> Handlers {
        Handlers {
            selector_handler: SelectorHandlerPairs::default(),
            response_handler: ResponseHandlers::default(),
        }
    }

    pub fn on_selector<F1, F2, Fut, K, S>(
        &mut self,
        selector: S,
        preprocessor: F1,
        processor: F2,
    ) -> &mut Self
    where
        F1: Fn(Arc<Context>, ElementRef) -> Option<K> + 'static + Sync + Send,
        F2: Fn(Arc<Context>, Arc<Crawler>, K) -> Fut + 'static + Sync + Send,
        Fut: Future<Output = ()> + 'static + Send,
        K: 'static + Send,
        S: AsRef<str> + Display,
    {
        self.selector_handler.push((
            Selector::parse(selector.as_ref())
                .unwrap_or_else(|_| panic!("Cannot Parse Selector {}", selector)),
            Box::new(move |a, b, c| {
                let k = preprocessor(a.clone(), c);
                k.map(|k| processor(a, b, k))
                    .map(|future| tokio::spawn(future));
            }),
        ));
        self
    }

    pub fn on_response<F1, F2, Fut, K>(&mut self, preprocessor: F1, processor: F2) -> &mut Self
    where
        F1: Fn(Arc<Context>, &Respond) -> Option<K> + 'static + Sync + Send,
        F2: Fn(Arc<Context>, Arc<Crawler>, K) -> Fut + 'static + Sync + Send,
        Fut: Future<Output = ()> + 'static + Send,
        K: 'static + Send,
    {
        self.response_handler
            .push(Box::new(move |ctx, crawler, resp| {
                let k = preprocessor(ctx.clone(), resp);
                k.map(|k| processor(ctx, crawler, k))
                    .map(|future| tokio::spawn(future));
            }));
        self
    }
}

pub trait Scraper: Sync + Send + Default + 'static {
    fn get_handlers(adder: &mut Handlers);

    fn starting_address() -> &'static str;

    fn thread_count() -> usize {
        16
    }

    fn user_agent() -> String {
        "testing_my_lib".to_string()
    }

    fn build_collector(name: &'static str) -> Option<Collector> {
        let url = Self::starting_address();
        let starting_address =
            Url::parse(url).unwrap_or_else(|x| panic!("Parsing {url} errored: {x}"));
        let mut constructor = Handlers::new();
        Self::get_handlers(&mut constructor);
        let handlers = Arc::new(constructor);

        let crawler = Arc::new(Crawler::new(
            Self::thread_count(),
            handlers,
            Arc::new(
                Client::builder()
                    .user_agent(Self::user_agent())
                    .build()
                    .ok()?,
            ),
        ));
        let running = Arc::new(AtomicBool::new(false));
        Some(Collector {
            crawler,
            running,
            starting_address,
            name,
        })
    }
}
