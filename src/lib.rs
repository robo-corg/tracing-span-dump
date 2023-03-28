use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tracing::{span, Subscriber};
use tracing_subscriber::Layer;

#[derive(Debug, Clone)]
pub struct SpanRecord {
    pub id: span::Id,
    pub parent: Option<span::Id>,
    pub metadata: &'static tracing::Metadata<'static>,
}

#[derive(Clone, Default)]
pub struct SpanSnapshot {
    spans: HashMap<span::Id, SpanRecord>,
}

impl SpanSnapshot {
    pub fn open_spans(&self) -> impl Iterator<Item = &SpanRecord> {
        self.spans.values()
    }

    pub fn dump_text(&self) {
        // let spans = open_spans()

        // let mut spans_read = self.spans.read().unwrap();

        // for (span_id, span) in spans_read.iter() {
        //     println!("{}", span.metadata.name());
        // }
    }

    fn new_span(&mut self, attrs: &span::Attributes<'_>, id: &span::Id) {
        self.spans.insert(
            id.clone(),
            SpanRecord {
                id: id.clone(),
                parent: attrs.parent().cloned(),
                metadata: attrs.metadata(),
            },
        );
    }

    fn close_span(&mut self, id: span::Id) {
        self.spans.remove(&id);
    }
}

#[derive(Clone)]
pub struct SpanDumpLayer {
    spans: Arc<RwLock<SpanSnapshot>>,
}

impl SpanDumpLayer {
    pub fn new() -> Self {
        SpanDumpLayer {
            spans: Arc::new(RwLock::new(Default::default())),
        }
    }

    pub fn snapshot(&self) -> SpanSnapshot {
        let spans_read = self.spans.read().unwrap();
        spans_read.clone()
    }
}

impl<S: Subscriber> Layer<S> for SpanDumpLayer {
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        let _ = (metadata, ctx);
        true
    }

    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut spans_write = self.spans.write().unwrap();
        spans_write.new_span(attrs, id);
    }

    fn on_record(
        &self,
        _span: &span::Id,
        _values: &span::Record<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_follows_from(
        &self,
        _span: &span::Id,
        _follows: &span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn event_enabled(
        &self,
        _event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        true
    }

    fn on_event(
        &self,
        _event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_enter(&self, _id: &span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}

    fn on_exit(&self, _id: &span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}

    fn on_close(&self, id: span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut spans_write = self.spans.write().unwrap();
        spans_write.close_span(id);
    }

    fn on_id_change(
        &self,
        _old: &span::Id,
        _new: &span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;
    use tracing::{info_span, Instrument};
    use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    use super::*;

    #[test]
    fn test_with_span() {
        let span_dumper = SpanDumpLayer::new();

        let _sub = tracing_subscriber::registry()
            .with(span_dumper.clone())
            .set_default();

        let spans = {
            let s = info_span!("test");
            let _s_guard = s.enter();
            span_dumper
                .snapshot()
                .open_spans()
                .cloned()
                .collect::<Vec<_>>()
        };

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].metadata.name(), "test");

        let spans_after_exit = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(spans_after_exit.len(), 0);
    }

    #[test]
    fn test_instrumented() {
        let span_dumper = SpanDumpLayer::new();

        let _sub = tracing_subscriber::registry()
            .with(span_dumper.clone())
            .set_default();

        let s = info_span!("test");

        let fut = async { futures::future::pending::<()>().await }.instrument(s);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 1);

        drop(fut);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 0);
    }

    #[tokio::test]
    async fn test_instrumented_inner() {
        let span_dumper = SpanDumpLayer::new();

        let _sub = tracing_subscriber::registry()
            .with(span_dumper.clone())
            .set_default();

        let mut fut = async {
            let s = info_span!("test");
            futures::future::pending::<()>().instrument(s).await
        }
        .boxed();

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 0);

        let _ = futures::poll!(&mut fut);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].metadata.name(), "test");

        drop(fut);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 0);
    }

    #[tokio::test]
    async fn test_instrumented_inner_completed() {
        let span_dumper = SpanDumpLayer::new();

        let _sub = tracing_subscriber::registry()
            .with(span_dumper.clone())
            .set_default();

        let mut fut = async {
            let s = info_span!("test");
            futures::future::ready(()).instrument(s).await;
            futures::future::pending::<()>().await;
        }
        .boxed();

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 0);

        let _ = futures::poll!(&mut fut);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 0);

        drop(fut);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 0);
    }

    #[tokio::test]
    async fn test_nested_instrumented() {
        let span_dumper = SpanDumpLayer::new();

        let _sub = tracing_subscriber::registry()
            .with(span_dumper.clone())
            .set_default();

        let mut fut = async {
            futures::future::pending::<()>()
                .instrument(info_span!("test_inner"))
                .await
        }
        .instrument(info_span!("test_outer"))
        .boxed();

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 1);

        let _ = futures::poll!(&mut fut);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        dbg!(&spans);
        assert_eq!(spans.len(), 2);

        drop(fut);

        let spans = span_dumper
            .snapshot()
            .open_spans()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 0);
    }
}
