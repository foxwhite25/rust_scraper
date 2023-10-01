use crate::{Context, Handlers, PageType};
use backoff::ExponentialBackoff;
use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use log::{error, warn};
use mime::Mime;
use reqwest::header::HeaderMap;
use reqwest::{Client, Response, StatusCode};
use scraper::Html;
use std::sync::Arc;
use tokio::sync::Semaphore;
use url::Url;

pub struct Respond {
    pub url: Url,
    pub content_length: Option<u64>,
    pub header: HeaderMap,
    pub status: StatusCode,
    pub bytes: Bytes,
}

impl Respond {
    async fn from(value: Response) -> Option<Self> {
        Some(Self {
            url: value.url().clone(),
            content_length: value.content_length().take(),
            header: value.headers().clone(),
            status: value.status(),
            bytes: value.bytes().await.ok()?,
        })
    }

    fn text(&self) -> String {
        let content_type = self
            .header
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        let encoding_name = content_type
            .as_ref()
            .and_then(|mime| mime.get_param("charset").map(|charset| charset.as_str()))
            .unwrap_or("utf-8");
        let encoding = Encoding::for_label(encoding_name.as_bytes()).unwrap_or(UTF_8);
        let (text, _, _) = encoding.decode(&self.bytes);
        text.into_owned()
    }
}

pub struct Crawler {
    pub(crate) semaphore: Arc<Semaphore>,
    pub(crate) max_permit: usize,
    handlers: Arc<Handlers>,
    client: Arc<Client>,
}

impl Crawler {
    pub fn new(max_permit: usize, handlers: Arc<Handlers>, client: Arc<Client>) -> Crawler {
        Crawler {
            semaphore: Arc::from(Semaphore::new(max_permit)),
            max_permit,
            handlers,
            client,
        }
    }

    pub async fn visit(self: Arc<Self>, url: &Url, page_type: PageType) {
        let semaphore = self.semaphore.clone();
        let client = self.client.clone();
        let handlers = self.handlers.clone();
        self.visit_worker(url, page_type, semaphore, handlers, client)
            .await
    }

    async fn visit_worker(
        self: Arc<Self>,
        url: &Url,
        page_type: PageType,
        semaphore: Arc<Semaphore>,
        handlers: Arc<Handlers>,
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
                let fragment = Html::parse_document(&respond.text());
                let ctx = Arc::new(Context {
                    page_type,
                    current_address: respond.url.clone(),
                });

                handlers
                    .response_handler
                    .iter()
                    .for_each(|handler| handler(ctx.clone(), self.clone(), &respond));

                handlers
                    .selector_handler
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
