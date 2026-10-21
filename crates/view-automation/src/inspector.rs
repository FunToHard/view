//! Standalone inspector HTML built exclusively from the public snapshot response.
use serde_json::Value;
use std::fmt::Write;
/// No scripts, network resources or privileged inspection API. Labels are escaped.
pub fn html(snapshot: &Value, trace: &[String]) -> String {
    let mut out = String::from(
        "<!doctype html><meta charset=utf-8><title>view inspector</title><style>body{background:#111827;color:#e5e7eb;font:14px system-ui}svg{background:#1f2937;border:1px solid #64748b;max-width:100%}rect{fill:none;stroke:#38bdf8}text{fill:#fff;font-size:12px}pre{white-space:pre-wrap}table{border-collapse:collapse}td,th{padding:6px;border:1px solid #475569}</style><h1>view inspector</h1>",
    );
    let _ = write!(
        out,
        "<p>Session {} · committed revision {}</p>",
        escape(snapshot["session"].as_str().unwrap_or("")),
        snapshot["revision"]
    );
    if let Some(windows) = snapshot["windows"].as_array() {
        for window in windows {
            let id = window["id"].as_str().unwrap_or("");
            let _ = write!(
                out,
                "<h2>Window {}</h2><p>Focus {} · capture {}</p><svg viewBox=\"0 0 1000 800\" width=1000 height=800 role=img aria-label=\"Committed layout bounds\">",
                escape(id),
                escape(&window["focus"].to_string()),
                escape(&window["capture"].to_string())
            );
            if let Some(nodes) = snapshot["nodes"].as_array() {
                for node in nodes.iter().filter(|n| n["window"] == id) {
                    let Some(b) = node["global_bounds"].as_array() else {
                        continue;
                    };
                    if b.len() != 4 {
                        continue;
                    }
                    let v: Vec<f64> = b.iter().map(|v| v.as_f64().unwrap_or(0.)).collect();
                    if let Some(clip) = node["effective_clip"].as_array()
                        && clip.len() == 4
                    {
                        let c: Vec<f64> = clip.iter().map(|v| v.as_f64().unwrap_or(0.)).collect();
                        let _ = write!(
                            out,
                            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" style=\"stroke:#fbbf24;stroke-dasharray:4 4\"><title>Accumulated clip</title></rect>",
                            c[0],
                            c[1],
                            c[2].max(0.),
                            c[3].max(0.)
                        );
                    }
                    let label = node["name"]
                        .as_str()
                        .unwrap_or_else(|| node["key"].as_str().unwrap_or(""));
                    if let Some(point) = node["actionable_point"].as_array()
                        && point.len() == 2
                    {
                        let _ = write!(
                            out,
                            "<circle cx=\"{}\" cy=\"{}\" r=4 fill=\"#4ade80\"><title>Verified hit point</title></circle>",
                            point[0].as_f64().unwrap_or(0.),
                            point[1].as_f64().unwrap_or(0.)
                        );
                    }
                    let _ = write!(
                        out,
                        "<g><title>{}</title><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/><text x=\"{}\" y=\"{}\">{}</text></g>",
                        escape(&node.to_string()),
                        v[0],
                        v[1],
                        v[2].max(0.),
                        v[3].max(0.),
                        v[0] + 2.,
                        v[1] + 12.,
                        escape(label)
                    );
                }
            }
            out.push_str("</svg>");
        }
    }
    out.push_str("<h2>Tree, clips, semantics and owned resources</h2><p>Bounds are diagnostic overlays. Actionable points are verified by the runtime hit query; a bounding box is not a click guarantee.</p><pre>");
    out.push_str(&escape(
        &serde_json::to_string_pretty(snapshot).unwrap_or_default(),
    ));
    out.push_str("</pre><h2>Why updated</h2><pre>");
    out.push_str(&escape(&trace.join("\n")));
    out.push_str("</pre>");
    out
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
