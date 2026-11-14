use super::{Briefing, Feed};
use crate::publication::local::escape;

pub(super) fn text(model: &Briefing, current: bool) -> String {
    let mut text = format!(
        "{}\nAs of: {}\nSource snapshot: {}\nPublication approval: not granted\n\n",
        model.title,
        model.as_of,
        if current {
            "current selected local bytes"
        } else {
            "STALE"
        }
    );
    for chapter in &model.chapters {
        text.push_str(&format!("{}\n{}\n\n", chapter.title, chapter.text));
        for source in &chapter.sources {
            text.push_str(&format!(
                "Source: {} lines {}–{}\n",
                source.source_id, source.start_line, source.end_line
            ));
        }
        text.push('\n');
    }
    text
}

pub(super) fn html(model: &Briefing, current: bool) -> String {
    let mut html = format!("<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>body{{font:1.1rem/1.6 system-ui,sans-serif;max-width:48rem;margin:2rem auto;padding:0 1rem}}pre{{font:inherit;white-space:pre-wrap;overflow-wrap:anywhere}}a{{color:#124b91}}audio{{max-width:100%}}</style></head><body><main><h1>{}</h1><p>As of: {}. Source snapshot: {}. Publication approval: not granted.</p>\n",
        escape(&model.language),escape(&model.title),escape(&model.title),escape(&model.as_of),
        if current {"current selected local bytes; remote revisions were not fetched"} else {"STALE"});
    if model.narration.is_some() {
        html.push_str("<audio controls preload=\"none\" src=\"narration.wav\">Supplied narration. The complete transcript follows.</audio><p>Supplied narration and any chapter timing are operator assertions, pending content and alignment review.</p>\n");
    }
    html.push_str("<nav aria-label=\"Chapters\"><ol>");
    for chapter in &model.chapters {
        html.push_str(&format!(
            "<li><a href=\"#chapter-{}\">{}</a></li>",
            escape(&chapter.id),
            escape(&chapter.title)
        ));
    }
    html.push_str("</ol></nav>\n");
    for chapter in &model.chapters {
        html.push_str(&format!("<section aria-labelledby=\"chapter-{}\"><h2 id=\"chapter-{}\">{}</h2><pre>{}</pre><ul>",
            escape(&chapter.id),escape(&chapter.id),escape(&chapter.title),escape(&chapter.text)));
        for location in &chapter.sources {
            html.push_str(&format!(
                "<li>Source {}: lines {}–{}</li>",
                escape(&location.source_id),
                location.start_line,
                location.end_line
            ));
        }
        html.push_str("</ul></section>\n");
    }
    html.push_str("<section><h2>Canonical sources</h2><ul>");
    for source in &model.sources {
        html.push_str(&format!(
            "<li>{}: revision {}; SHA-256 {}",
            escape(&source.id),
            escape(&source.revision),
            escape(&source.sha256)
        ));
        if let Some(url) = &source.canonical_url {
            html.push_str(&format!(
                " — <a href=\"{}\" rel=\"noreferrer\">canonical source</a>",
                escape(url)
            ));
        }
        html.push_str("</li>");
    }
    html.push_str(&format!(
        "</ul></section><p>Rights declaration: {}</p></main></body></html>\n",
        escape(&model.rights)
    ));
    html
}

pub(super) fn feed(
    model: &Briefing,
    feed: &Feed,
    input_sha256: &str,
    current: bool,
    audio_bytes: Option<usize>,
) -> String {
    let mut rss = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:rf=\"urn:renderflow:briefing:v1\"><channel><title>{}</title><link>{}</link><description>{}</description><language>{}</language><item><title>{}</title><link>{}</link><guid isPermaLink=\"false\">renderflow:briefing:{}:{}</guid><description>{}</description><rf:asOf>{}</rf:asOf><rf:freshness>{}</rf:freshness><rf:publicationApproval>not_granted</rf:publicationApproval>",
        escape(&feed.title),escape(&feed.site_url),escape(&feed.description),escape(&model.language),
        escape(&model.title),escape(&feed.episode_url),escape(&model.id),input_sha256,
        escape(&text(model,current)),escape(&model.as_of),if current {"current_local_snapshot"} else {"stale"});
    if let (Some(url), Some(size)) = (&feed.narration_url, audio_bytes) {
        rss.push_str(&format!(
            "<enclosure url=\"{}\" length=\"{}\" type=\"audio/wav\" />",
            escape(url),
            size
        ));
    }
    rss.push_str("</item></channel></rss>\n");
    rss
}
