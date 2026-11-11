//! Bounded attribute edits. Unchanged XML remains byte-for-byte intact in API
//! proposals; native callers can also preserve their document node identities.
use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use sivgo_dom::{parse_svg, SvgDocument};

use crate::{validate_proposal, AgentError, MAX_SVG_BYTES};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectEdit {
    pub id: String,
    pub attributes: BTreeMap<String, Option<String>>,
}

/// Resolve a transaction against XML IDs. All edits succeed or nothing changes.
pub fn resolve_object_edits(source: &str, edits: &[ObjectEdit]) -> Result<String, AgentError> {
    validate_proposal(source)?;
    if edits.is_empty() || edits.len() > 100 {
        return Err(AgentError::Response);
    }
    let xml = roxmltree::Document::parse(source).map_err(|_| AgentError::Svg)?;
    let mut ids = HashSet::new();
    let mut patches = Vec::new();
    for edit in edits {
        if edit.id.is_empty()
            || edit.id.len() > 256
            || !ids.insert(&edit.id)
            || edit.attributes.is_empty()
            || edit.attributes.len() > 32
        {
            return Err(AgentError::Response);
        }
        let mut targets = xml
            .descendants()
            .filter(|node| node.is_element() && node.attribute("id") == Some(edit.id.as_str()));
        let target = targets.next().ok_or(AgentError::Response)?;
        if targets.next().is_some() {
            return Err(AgentError::Response);
        }
        let mut additions = String::new();
        for (name, value) in &edit.attributes {
            // Qualified attributes and identity/namespace changes require a full
            // proposal. Restrict names before interpolating them into raw XML.
            if name.len() > 64
                || !name.bytes().next().is_some_and(|b| b.is_ascii_alphabetic())
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
                || matches!(name.as_str(), "id" | "xmlns")
                || value.as_ref().is_some_and(|value| value.len() > 16 * 1024)
            {
                return Err(AgentError::Response);
            }
            let replacement = value
                .as_ref()
                .map(|value| format!("{name}=\"{}\"", escape_attribute(value)))
                .unwrap_or_default();
            if let Some(attribute) = target
                .attributes()
                .find(|attr| attr.namespace().is_none() && attr.name() == name)
            {
                patches.push((attribute.range(), replacement));
            } else if value.is_some() {
                additions.push(' ');
                additions.push_str(&replacement);
            }
        }
        if !additions.is_empty() {
            // Locate the end of the opening tag while respecting quoted >.
            let start = target.range().start;
            let mut quote = None;
            let end = source.as_bytes()[start..]
                .iter()
                .position(|&b| {
                    if let Some(active) = quote {
                        if b == active {
                            quote = None;
                        }
                        false
                    } else if b == b'\'' || b == b'"' {
                        quote = Some(b);
                        false
                    } else {
                        b == b'>'
                    }
                })
                .ok_or(AgentError::Svg)?
                + start;
            let end = if source.as_bytes()[end - 1] == b'/' {
                end - 1
            } else {
                end
            };
            patches.push((end..end, additions));
        }
    }
    patches.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut result = source.to_owned();
    for (range, replacement) in patches {
        result.replace_range(range, &replacement);
        if result.len() > MAX_SVG_BYTES {
            return Err(AgentError::Svg);
        }
    }
    validate_proposal(&result)?;
    Ok(result)
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
        .replace('\t', "&#9;")
}

/// A bounded, plain-text review using the original XML attribute values.
pub fn review_object_edits(source: &str, edits: &[ObjectEdit]) -> Vec<String> {
    let mut lines = vec![format!(
        "{} object edit(s). Other objects are preserved.",
        edits.len()
    )];
    let Ok(xml) = roxmltree::Document::parse(source) else {
        return lines;
    };
    let shorten = |value: &str| value.chars().take(120).collect::<String>();
    let total: usize = edits.iter().map(|edit| edit.attributes.len()).sum();
    for edit in edits {
        let node = xml
            .descendants()
            .find(|node| node.is_element() && node.attribute("id") == Some(edit.id.as_str()));
        for (name, value) in &edit.attributes {
            if lines.len() > 100 {
                lines.push(format!("{} more attribute change(s).", total - 100));
                return lines;
            }
            let before = node
                .and_then(|node| node.attribute(name.as_str()))
                .map(shorten)
                .unwrap_or_else(|| "(unset)".into());
            let after = value
                .as_deref()
                .map(shorten)
                .unwrap_or_else(|| "(remove)".into());
            lines.push(format!("#{} · {}: {} → {}", edit.id, name, before, after));
        }
    }
    lines
}

/// Apply attribute-only edits to a native document using a snapshot with the
/// same tree (which may contain temporary selection XML IDs). Preserve all
/// internal IDs, untouched nodes, editor metadata and parent/child links.
pub fn apply_object_edits(
    document: &SvgDocument,
    snapshot: &str,
    edits: &[ObjectEdit],
) -> Result<SvgDocument, AgentError> {
    let revised =
        parse_svg(&resolve_object_edits(snapshot, edits)?).map_err(|_| AgentError::Svg)?;
    let snapshot_doc = parse_svg(snapshot).map_err(|_| AgentError::Svg)?;
    let nodes = |doc: &SvgDocument| {
        doc.root()
            .map(|root| doc.traverse_pre_order(root))
            .ok_or(AgentError::Svg)
    };
    let original_ids = nodes(document)?;
    let snapshot_ids = nodes(&snapshot_doc)?;
    let revised_ids = nodes(&revised)?;
    if original_ids.len() != snapshot_ids.len() || snapshot_ids.len() != revised_ids.len() {
        return Err(AgentError::Svg);
    }
    let mut result = document.clone();
    for ((original_id, snapshot_id), revised_id) in
        original_ids.into_iter().zip(snapshot_ids).zip(revised_ids)
    {
        let before = snapshot_doc.get_node(snapshot_id).ok_or(AgentError::Svg)?;
        if !edits
            .iter()
            .any(|edit| before.xml_id.as_ref() == Some(&edit.id))
        {
            continue;
        }
        let after = revised.get_node(revised_id).ok_or(AgentError::Svg)?;
        let node = result.get_node_mut(original_id).ok_or(AgentError::Svg)?;
        if std::mem::discriminant(&node.element) != std::mem::discriminant(&after.element) {
            return Err(AgentError::Svg);
        }
        node.element = after.element.clone();
        node.presentation = after.presentation.clone();
        node.attributes = after.attributes.clone();
        for name in edits
            .iter()
            .filter(|edit| before.xml_id.as_ref() == Some(&edit.id))
            .flat_map(|edit| edit.attributes.keys())
        {
            result.notify_attribute_changed(original_id, name);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"><!-- keep --><g id="layer"><rect id="box" width="10" fill='red' data-note="a > b"/><circle id="other" r="2"/></g></svg>"#;

    fn edits(value: serde_json::Value) -> Vec<ObjectEdit> {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn object_edit_preserves_unrelated_xml_and_native_identities() {
        let edits = edits(
            serde_json::json!([{"id":"box","attributes":{"fill":"blue","opacity":"0.5","width":null,"data-note":"中\n<&\""}}]),
        );
        let svg = resolve_object_edits(SVG, &edits).unwrap();
        assert!(svg.contains("<!-- keep -->"));
        assert!(svg.contains(r#"<circle id="other" r="2"/>"#));
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let node = xml
            .descendants()
            .find(|n| n.attribute("id") == Some("box"))
            .unwrap();
        assert_eq!(node.attribute("fill"), Some("blue"));
        assert_eq!(node.attribute("data-note"), Some("中\n<&\""));
        assert_eq!(node.attribute("width"), None);
        let doc = parse_svg(SVG).unwrap();
        let changed = apply_object_edits(&doc, SVG, &edits).unwrap();
        assert_eq!(doc.root(), changed.root());
        for id in ["layer", "box", "other"] {
            assert_eq!(doc.find_by_xml_id(id), changed.find_by_xml_id(id));
        }
        let fill = &changed
            .get_node(changed.find_by_xml_id("box").unwrap())
            .unwrap()
            .presentation
            .fill;
        assert_eq!(
            fill,
            &Some(sivgo_dom::Paint::Color(sivgo_dom::Color::rgb(0, 0, 255)))
        );
    }

    #[test]
    fn rejects_ambiguous_missing_unsafe_and_conflicting_edits_atomically() {
        for value in [
            serde_json::json!([{"id":"missing","attributes":{"fill":"blue"}}]),
            serde_json::json!([{"id":"box","attributes":{"id":"new"}}]),
            serde_json::json!([{"id":"box","attributes":{"xmlns":"evil"}}]),
            serde_json::json!([{"id":"box","attributes":{"x\" onload":"evil"}}]),
            serde_json::json!([{"id":"box","attributes":{"onload":"evil"}}]),
            serde_json::json!([{"id":"box","attributes":{"fill":"url(https://example.com)"}}]),
            serde_json::json!([{"id":"box","attributes":{"fill":"blue"}},{"id":"box","attributes":{"fill":"red"}}]),
        ] {
            assert!(resolve_object_edits(SVG, &edits(value)).is_err());
        }
        let duplicate = SVG.replace("id=\"other\"", "id=\"box\"");
        assert!(resolve_object_edits(
            &duplicate,
            &edits(serde_json::json!([{"id":"box","attributes":{"fill":"blue"}}]))
        )
        .is_err());
    }

    #[test]
    fn temporary_selection_ids_do_not_leak_into_native_edits() {
        let mut doc = parse_svg(SVG).unwrap();
        let id = doc.find_by_xml_id("box").unwrap();
        let node = doc.get_node_mut(id).unwrap();
        node.xml_id = None;
        node.locked = true;
        let mut snapshot = doc.clone();
        snapshot.get_node_mut(id).unwrap().xml_id = Some("temporary-selection".into());
        let snapshot = sivgo_dom::serialize_svg(&snapshot);
        let operations =
            edits(serde_json::json!([{"id":"temporary-selection","attributes":{"fill":"blue"}}]));
        let updated = apply_object_edits(&doc, &snapshot, &operations).unwrap();
        let node = updated.get_node(id).unwrap();
        assert!(node.xml_id.is_none());
        assert!(node.locked);
        assert_eq!(node.parent, doc.get_node(id).unwrap().parent);
        assert_eq!(node.children, doc.get_node(id).unwrap().children);
        assert_eq!(
            node.presentation.fill,
            Some(sivgo_dom::Paint::Color(sivgo_dom::Color::rgb(0, 0, 255)))
        );
        assert!(review_object_edits(&snapshot, &operations)
            .iter()
            .any(|line| line.contains("fill: #ff0000 → blue")));
    }
}
