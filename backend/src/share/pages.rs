//! The small standalone pages behind shared links.

use std::fmt::Write as _;

use super::{escape, pretty_date, profile_headline, visit_headline, SharedVisit};
use crate::game::Summary;

/// A small standalone page with link-preview tags, outside the web app.
pub fn page(title: &str, description: &str, image: &str, url: &str, body: &str) -> String {
    let (title, description) = (escape(title), escape(description));
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Mangiamundo</title>
<meta name="description" content="{description}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Mangiamundo">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:image" content="{image}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:url" content="{url}">
<meta name="twitter:card" content="summary_large_image">
<link rel="icon" href="/icon.svg">
<style>
  body {{ margin: 0; background: #f2f5f8; color: #14263a; font: 16px/1.5 system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; }}
  main {{ max-width: 640px; margin: 0 auto; padding: 24px 16px 48px; }}
  img {{ width: 100%; height: auto; border-radius: 14px; display: block; }}
  h1 {{ font-size: 26px; margin: 20px 0 4px; }}
  .muted {{ color: #5a6a7d; }}
  ul {{ list-style: none; padding: 0; margin: 16px 0; display: grid; gap: 6px; }}
  li {{ background: #fff; border-radius: 10px; padding: 10px 14px; display: flex; justify-content: space-between; gap: 12px; }}
  h2 {{ font-size: 18px; margin: 24px 0 0; }}
  a.start {{ display: inline-block; margin-top: 24px; background: #2873c2; color: #fff; font-weight: 800; padding: 10px 18px; border-radius: 12px; text-decoration: none; box-shadow: 0 3px 0 #1d5a9e; }}
</style>
</head>
<body><main>
<img src="{image}" alt="{title}" width="1200" height="630">
{body}
<a class="start" href="/">Start your own map</a>
</main></body>
</html>
"#
    )
}

/// The public page's content: level, counts per continent and earned badges. No places
/// or dates.
pub fn profile_body(summary: &Summary, area: &str) -> String {
    let mut body = format!(
        "<h1>{}</h1>\n<p class=\"muted\">{} · Level {} · {} places visited</p>\n<ul>\n",
        escape(&profile_headline(summary)),
        escape(area),
        summary.profile.level,
        summary.places_visited,
    );
    for continent in summary.continents.iter().filter(|c| c.available > 0) {
        let _ = writeln!(
            body,
            "<li><span>{}</span><span class=\"muted\">{} of {}</span></li>",
            escape(&continent.name),
            continent.discovered,
            continent.available
        );
    }
    body.push_str("</ul>\n");
    let earned: Vec<_> = summary.badges.iter().filter(|b| b.earned).collect();
    if !earned.is_empty() {
        body.push_str("<h2>Badges</h2>\n<ul>\n");
        for badge in earned {
            let _ = writeln!(
                body,
                "<li><strong>{}</strong><span class=\"muted\">{}</span></li>",
                escape(&badge.title),
                escape(&badge.description)
            );
        }
        body.push_str("</ul>\n");
    }
    body
}

pub fn visit_body(visit: &SharedVisit) -> String {
    format!(
        "<h1>{}</h1>\n<p class=\"muted\">{}</p>\n",
        escape(&visit_headline(visit)),
        pretty_date(&visit.date)
    )
}
