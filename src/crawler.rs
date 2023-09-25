use crate::{Handlers};
use backoff::ExponentialBackoff;
use log::{error, warn};
use reqwest::header::HeaderMap;
use reqwest::{Client, Response, StatusCode};
use scraper::Html;
use std::sync::{Arc};
use tokio::sync::Semaphore;
use url::Url;

pub struct Respond {
    pub url: Url,
    pub content_length: Option<u64>,
    pub header: HeaderMap,
    pub status: StatusCode,
    pub text: String,
}

impl Respond {
    async fn from(value: Response) -> Option<Self> {
        Some(Self {
            url: value.url().clone(),
            content_length: value.content_length().take(),
            header: value.headers().clone(),
            status: value.status(),
            text: value.text().await.ok()?,
        })
    }
}

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
    handlers: Arc<Handlers<T>>,
    custom_state: Arc<T>,
    client: Arc<Client>,
}

impl<T: 'static + Sync + Send> Crawler<T> {
    pub fn new(
        max_permit: usize,
        handlers: Arc<Handlers<T>>,
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
        let custom_state = self.custom_state.clone();
        let client = self.client.clone();
        let handlers = self.handlers.clone();
        self.visit_worker(
            url,
            page_type,
            semaphore,
            handlers,
            custom_state,
            client,
        )
        .await
    }

    async fn visit_worker(
        self: Arc<Self>,
        url: Url,
        page_type: PageType,
        semaphore: Arc<Semaphore>,
        handlers: Arc<Handlers<T>>,
        custom_state: Arc<T>,
        client: Arc<Client>,
    ) {
        let _permit = semaphore.acquire().await.unwrap();
        let url_str = url.as_str();
        match backoff::future::retry(ExponentialBackoff::default(), || async {
            Ok(client.get(url_str).send().await?)
        })
        .await
        {
            Ok(k) => {
                let resp = Respond::from(k).await;
                let Some(respond) = resp else {
                    warn!("Invalid Content type at {}", url_str);
                    return
                };
                let respond = Arc::new(respond);
                let fragment = Html::parse_document(respond.text.as_str());
                let ctx = Arc::new(Context {
                    state: custom_state,
                    page_type,
                    current_address: respond.url.clone(),
                });

                handlers.response_handler
                    .iter()
                    .for_each(|handler| handler(ctx.clone(), self.clone(), respond.clone()));

                handlers.selector_handler
                    .iter()
                    .map(|(sel, handler)| (fragment.select(sel), handler))
                    .for_each(|(select, handler)| {
                        select.for_each(|element| handler(ctx.clone(), self.clone(), element))
                    });
            }
            Err(err) => {
                error!("{}", err)
            }
        };
    }
}
