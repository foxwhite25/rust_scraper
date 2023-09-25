use crate::{Context, PageType};
use crate::{Crawler, Respond};
use reqwest::{Client, Response};
use scraper::{ElementRef, Selector};
use std::fmt::Display;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::time::Duration;
use tokio::time::sleep;
use url::Url;

type SelectorHandler<T> =
    Box<dyn Fn(Arc<Context<T>>, Arc<Crawler<T>>, ElementRef) + Sync + Send>;
type ResponseHandler<T> =
    Box<dyn Fn(Arc<Context<T>>, Arc<Crawler<T>>, Arc<Respond>) + Sync + Send>;
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
pub struct Handlers<T: Sync + Send> {
    pub(crate) selector_handler: SelectorHandlerPairs<T>,
    pub(crate) response_handler: ResponseHandlers<T>,
}

impl<T: Sync + Send + 'static> Handlers<T> {
    pub fn new() -> Handlers<T> {
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
        F1: Fn(Arc<Context<T>>, ElementRef) -> Option<K> + 'static + Sync + Send,
        F2: Fn(Arc<Context<T>>, Arc<Crawler<T>>, K) -> Fut + 'static + Sync + Send,
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
        F1: Fn(Arc<Context<T>>, Arc<Respond>) -> Option<K> + 'static + Sync + Send,
        F2: Fn(Arc<Context<T>>, Arc<Crawler<T>>, K) -> Fut + 'static + Sync + Send,
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

pub trait Scraper: Sized + Sync + Send + Default + 'static {
    fn get_handlers(adder: &mut Handlers<Self>);

    fn thread_count() -> usize {
        16
    }

    fn user_agent() -> String {
        "testing_my_lib".to_string()
    }

    fn build_collector() -> Option<Collector<Self>> {
        let mut constructor = Handlers::new();
        Self::get_handlers(&mut constructor);
        let handlers = Arc::new(constructor);

        let crawler = Arc::new(Crawler::new(
            Self::thread_count(),
            handlers,
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
