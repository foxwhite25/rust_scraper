use scraper::ElementRef;
use url::Url;

#[derive(Default)]
pub struct NewsData {}
#[derive(Default)]
pub struct PersonData {}
#[derive(Default)]
pub struct ReportData {}

pub enum PageType {
    Index,
    News(Box<NewsData>),
    Person(Box<PersonData>),
    Report(Box<ReportData>),
}

pub struct Context {
    pub page_type: PageType,
    pub current_address: Url,
}

impl Context {
    pub fn parse_href(&self, element: ElementRef) -> Option<Url> {
        self.current_address
            .join(element.value().attr("href")?)
            .ok()
    }

    pub fn parse_text(&self, element: ElementRef) -> Option<Url> {
        self.current_address
            .join(element.text().collect::<String>().as_str())
            .ok()
    }
}

impl PageType {
    pub fn news() -> Self {
        Self::news_with(Default::default())
    }

    pub fn news_with(data: NewsData) -> Self {
        PageType::News(Box::new(data))
    }

    pub fn person() -> Self {
        Self::person_with(Default::default())
    }

    pub fn person_with(data: PersonData) -> Self {
        PageType::Person(Box::new(data))
    }

    pub fn report() -> Self {
        Self::report_with(Default::default())
    }

    pub fn report_with(data: ReportData) -> Self {
        PageType::Report(Box::new(data))
    }

    pub fn index() -> Self {
        PageType::Index
    }
}
