use crate::Crawler;
use crate::{Context, PageType};
use reqwest::{Client, Response};
use scraper::{ElementRef, Html, Selector};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::sleep;
use url::Url;

type SelectorHandler<T> = Box<
    dyn Fn(Arc<RwLock<Context<T>>>, Arc<Crawler<T>>, ElementRef)
        + Sync
        + Send,
>;
type ResponseHandler<T> = Box<
    dyn Fn(&mut Context<T>, Arc<Crawler<T>>, Response) -> Pin<Box<dyn Future<Output = ()>>>
        + Sync
        + Send,
>;
pub type SelectorHandlerPairs<T> = Vec<(Selector, SelectorHandler<T>)>;
pub type ResponseHandlers<T> = Vec<ResponseHandler<T>>;

pub struct Collector<T: Sync + Send + 'static> {
    crawler: Arc<Crawler<T>>,
    running: Arc<AtomicBool>,
}

impl<T: Sync + Send + 'static> Collector<T> {
    pub async fn start(&self, url: Url) -> Option<()> {
        let crawler = self.crawler.clone();
        let sem = crawler.semaphore.clone();
        let thread = self.crawler.max_permit;

        crawler.visit(url, PageType::Index).await;
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
pub struct HandlerAdder<T: Sync + Send> {
    selector_handler: SelectorHandlerPairs<T>,
    response_handler: ResponseHandlers<T>,
}

impl<T: Sync + Send> HandlerAdder<T> {
    pub fn new() -> HandlerAdder<T> {
        HandlerAdder {
            selector_handler: SelectorHandlerPairs::default(),
            response_handler: ResponseHandlers::default(),
        }
    }

    fn take(self) -> SelectorHandlerPairs<T> {
        self.selector_handler
    }

    async fn _selector_wrapper<'a, F, Fut>(f: F, ctx: &mut Context<T>, crawler: Arc<Crawler<T>>, sel: String)
    where
        F: Fn(&mut Context<T>, Arc<Crawler<T>>, ElementRef) -> Fut + 'static + Sync + Send,
        Fut: Future<Output = ()> + 'static + Sync + Send
    {
        let html = Html::parse_fragment(sel.as_str());
        f(ctx,crawler,html.root_element()).await;
    }

    pub fn on_selector<F>(&mut self, selector: &str, f: F) -> &mut Self
    where
        F: Fn(Arc<RwLock<Context<T>>>, Arc<Crawler<T>>, ElementRef) + 'static + Sync + Send,
    {
        self.selector_handler.push((
            Selector::parse(selector)
                .unwrap_or_else(|_| panic!("Cannot Parse Selector {}", selector)),
            Box::new(f),
        ));
        self
    }

    pub fn on_response<F, Fut>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&mut Context<T>, Arc<Crawler<T>>, Response) -> Fut + 'static + Sync + Send,
        Fut: Future<Output = ()> + 'static + Sync + Send,
    {
        self.response_handler
            .push(Box::new(move |ctx, crawler, resp| {
                Box::pin(f(ctx, crawler, resp))
            }));
        self
    }
}

pub trait Scraper: Sized + Sync + Send + Default + 'static {
    fn get_handlers(adder: &mut HandlerAdder<Self>);

    fn thread_count() -> usize {
        16
    }

    fn user_agent() -> String {
        "testing_my_lib".to_string()
    }

    fn build_collector() -> Option<Collector<Self>> {
        let mut constructor = HandlerAdder::new();
        Self::get_handlers(&mut constructor);
        let handler = Arc::new(constructor.take());
        let crawler = Arc::new(Crawler::new(
            Self::thread_count(),
            handler,
            Arc::new(Self::default()),
            Arc::new(
                Client::builder()
                    .user_agent(Self::user_agent())
                    .build()
                    .ok()?,
            ),
        ));
        let running = Arc::new(AtomicBool::new(false));
        Some(Collector { crawler, running })
    }
}
