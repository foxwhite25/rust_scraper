// use crate::{Crawler, PageType};
// use futures::future::BoxFuture;
// use std::sync::Arc;
// use url::Url;
//
// pub trait Visitable {
//     fn to_target(self) -> (Url, PageType);
// }
//
// impl<U> Visitable for (U, PageType)
// where
//     U: Into<Url>,
// {
//     fn to_target(self) -> (Url, PageType) {
//         (self.0.into(), self.1)
//     }
// }
//
// pub trait VisitExt: Iterator {
//     fn visit_via<T>(self, crawler: Arc<Crawler<T>>) -> Option<Vec<BoxFuture<'static, ()>>>
//     where
//         Self::Item: Visitable,
//         Self: Sized,
//         T: Send + Sync + 'static,
//     {
//         Some(
//             self.map(|visitable| visitable.to_target())
//                 .map(|(url, page_type)| crawler.clone().visit(url, page_type))
//                 .map(|x| Box::pin(x) as BoxFuture<'static, ()>)
//                 .collect(),
//         )
//     }
// }
//
// impl<I: Iterator> VisitExt for I {}
