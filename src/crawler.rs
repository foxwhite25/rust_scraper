use crate::SelectorHandlerPairs;
use backoff::ExponentialBackoff;
use log::{error, info, warn};
use reqwest::Client;
use scraper::Html;
use std::sync::{Arc, RwLock};
use tokio::sync::{Semaphore};
use url::Url;

pub struct NewsData {}

pub struct PersonData {}

pub struct ReportData {}

pub enum PageType {
    Index,
    News(Box<NewsData>),
    Person(Box<PersonData>),
    Report(Box<ReportData>),
}

pub struct Context<T: Sync + Send> {
    pub state: Arc<T>,
    pub page_type: PageType,
    pub current_address: Url,
}

pub struct Crawler<T: Sync + Send> {
    pub(crate) semaphore: Arc<Semaphore>,
    pub(crate) max_permit: usize,
    handlers: Arc<SelectorHandlerPairs<T>>,
    custom_state: Arc<T>,
    client: Arc<Client>,
}

impl<T: 'static + Sync + Send> Crawler<T> {
    pub fn new(
        max_permit: usize,
        handlers: Arc<SelectorHandlerPairs<T>>,
        custom_state: Arc<T>,
        client: Arc<Client>,
    ) -> Crawler<T> {
        Crawler {
            semaphore: Arc::from(Semaphore::new(max_permit)),
            max_permit,
            handlers,
            custom_state,
            client,
        }
    }

    pub async fn visit(self: Arc<Self>, url: Url, page_type: PageType) {
        let semaphore = self.semaphore.clone();
        let handlers = self.handlers.clone();
        let custom_state = self.custom_state.clone();
        let client = self.client.clone();
        self.visit_worker(url, page_type, semaphore, handlers, custom_state, client)
            .await
    }

    async fn visit_worker(
        self: Arc<Self>,
        url: Url,
        page_type: PageType,
        semaphore: Arc<Semaphore>,
        handlers: Arc<SelectorHandlerPairs<T>>,
        custom_state: Arc<T>,
        client: Arc<Client>,
    ) {
        let permit = semaphore.acquire_owned().await.unwrap();
        info!("Visiting {}", url);
        let url_str = url.as_str();
        match backoff::future::retry(ExponentialBackoff::default(), || async {
            Ok(client.get(url_str).send().await?)
        })
        .await
        {
            Ok(k) => {
                let url = k.url().clone();
                let Ok(text) = k.text().await else {
                    warn!("Invalid Content type at {}", url_str);
                    drop(permit);
                    return
                };
                let fragment = Html::parse_document(text.as_str());
                let ctx = Arc::new(RwLock::new(Context {
                    state: custom_state,
                    page_type,
                    current_address: url,
                }));
                handlers
                    .iter()
                    .map(|(sel, handler)| (fragment.select(sel), handler))
                    .for_each(|(htmls, handler)| {
                        htmls.for_each(|html| handler(ctx.clone(), self.clone(), html))
                    });
            }
            Err(err) => {
                error!("{}", err)
            }
        };
        drop(permit);
    }
}
