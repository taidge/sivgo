//! Commands for SVG clip path and mask object operations.

use sivgo_dom::{
    ClipPathAttrs, DefsAttrs, MaskAttrs, NodeId, PresentationAttrs, SvgDocument, SvgElement,
    SvgNode,
};

use crate::command::Command;

/// Object operation kind for clipping and masking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipMaskMode {
    Clip,
    Mask,
}

impl ClipMaskMode {
    fn resource_element(self) -> SvgElement {
        match self {
            Self::Clip => SvgElement::ClipPath(ClipPathAttrs::default()),
            Self::Mask => SvgElement::Mask(MaskAttrs::default()),
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Clip => "clip",
            Self::Mask => "mask",
        }
    }

    fn presentation_ref<'a>(self, pres: &'a PresentationAttrs) -> Option<&'a str> {
        match self {
            Self::Clip => pres.clip_path.as_deref(),
            Self::Mask => pres.mask.as_deref(),
        }
    }

    fn set_presentation_ref(self, pres: &mut PresentationAttrs, value: Option<String>) {
        match self {
            Self::Clip => pres.clip_path = value,
            Self::Mask => pres.mask = value,
        }
    }

    fn attr_name(self) -> &'static str {
        match self {
            Self::Clip => "clip-path",
            Self::Mask => "mask",
        }
    }
}

#[derive(Debug, Clone)]
struct TargetState {
    id: NodeId,
    presentation: PresentationAttrs,
    attributes: std::collections::HashMap<String, String>,
}

impl TargetState {
    fn capture(doc: &SvgDocument, id: NodeId) -> Option<Self> {
        let node = doc.get_node(id)?;
        Some(Self {
            id,
            presentation: node.presentation.clone(),
            attributes: node.attributes.clone(),
        })
    }

    fn restore(&self, doc: &mut SvgDocument) {
        if let Some(node) = doc.get_node_mut(self.id) {
            node.presentation = self.presentation.clone();
            node.attributes = self.attributes.clone();
            doc.notify_attribute_changed(self.id, "style");
        }
    }
}

/// Uses one selected source object as a clip path or mask for target objects.
#[derive(Debug)]
pub struct SetClipMaskCommand {
    mode: ClipMaskMode,
    target_ids: Vec<NodeId>,
    source_id: NodeId,
    source_parent: Option<NodeId>,
    source_index: usize,
    old_targets: Vec<TargetState>,
    defs_id: Option<NodeId>,
    created_defs: bool,
    resource_id: Option<NodeId>,
    resource_xml_id: Option<String>,
}

impl SetClipMaskCommand {
    pub fn new(mode: ClipMaskMode, target_ids: Vec<NodeId>, source_id: NodeId) -> Self {
        Self {
            mode,
            target_ids,
            source_id,
            source_parent: None,
            source_index: 0,
            old_targets: Vec::new(),
            defs_id: None,
            created_defs: false,
            resource_id: None,
            resource_xml_id: None,
        }
    }

    fn find_defs(doc: &SvgDocument) -> Option<NodeId> {
        let root = doc.root()?;
        doc.children(root).iter().copied().find(|id| {
            doc.get_node(*id)
                .map(|node| matches!(node.element, SvgElement::Defs(_)))
                .unwrap_or(false)
        })
    }

    fn ensure_defs(&mut self, doc: &mut SvgDocument) -> Option<NodeId> {
        if let Some(existing) = Self::find_defs(doc) {
            self.created_defs = false;
            self.defs_id = Some(existing);
            return Some(existing);
        }

        let root = doc.root()?;
        let defs_id = if let Some(previous) = self.defs_id {
            doc.insert_node(SvgNode::new(previous, SvgElement::Defs(DefsAttrs)));
            doc.insert_child_at(root, previous, 0);
            previous
        } else {
            let id = doc.add_node(SvgElement::Defs(DefsAttrs));
            doc.insert_child_at(root, id, 0);
            id
        };
        self.created_defs = true;
        self.defs_id = Some(defs_id);
        Some(defs_id)
    }

    fn ensure_resource(&mut self, doc: &mut SvgDocument, defs_id: NodeId) -> NodeId {
        let xml_id = self.resource_xml_id.clone().unwrap_or_else(|| {
            format!(
                "{}_{}",
                self.mode.prefix(),
                self.source_id.to_string().replace('-', "")[..8].to_string()
            )
        });

        let resource_id = if let Some(previous) = self.resource_id {
            let mut node = SvgNode::new(previous, self.mode.resource_element());
            node.xml_id = Some(xml_id.clone());
            doc.insert_node(node);
            doc.append_child(defs_id, previous);
            previous
        } else {
            let id = doc.add_node(self.mode.resource_element());
            if let Some(node) = doc.get_node_mut(id) {
                node.xml_id = Some(xml_id.clone());
            }
            doc.append_child(defs_id, id);
            id
        };

        self.resource_id = Some(resource_id);
        self.resource_xml_id = Some(xml_id);
        resource_id
    }
}

impl Command for SetClipMaskCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        if self.target_ids.is_empty() || doc.get_node(self.source_id).is_none() {
            return;
        }

        if self.old_targets.is_empty() {
            self.old_targets = self
                .target_ids
                .iter()
                .filter_map(|&id| TargetState::capture(doc, id))
                .collect();
        }

        let Some(defs_id) = self.ensure_defs(doc) else {
            return;
        };
        let resource_id = self.ensure_resource(doc, defs_id);

        if let Some((parent, index)) = doc.detach_node(self.source_id) {
            self.source_parent = Some(parent);
            self.source_index = index;
        }
        doc.append_child(resource_id, self.source_id);

        let Some(resource_xml_id) = self.resource_xml_id.clone() else {
            return;
        };
        let reference = format!("url(#{resource_xml_id})");
        for &target_id in &self.target_ids {
            if let Some(node) = doc.get_node_mut(target_id) {
                self.mode
                    .set_presentation_ref(&mut node.presentation, Some(reference.clone()));
                node.attributes.remove(self.mode.attr_name());
                doc.notify_attribute_changed(target_id, self.mode.attr_name());
            }
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        for target in &self.old_targets {
            target.restore(doc);
        }

        if let Some(resource_id) = self.resource_id {
            doc.detach_node(self.source_id);
            if let Some(parent) = self.source_parent {
                doc.insert_child_at(parent, self.source_id, self.source_index);
            }
            doc.remove_node(resource_id);
        }

        if self.created_defs {
            if let Some(defs_id) = self.defs_id {
                if doc.children(defs_id).is_empty() {
                    doc.remove_node(defs_id);
                }
            }
        }
    }

    fn description(&self) -> &str {
        match self.mode {
            ClipMaskMode::Clip => "Set clip",
            ClipMaskMode::Mask => "Set mask",
        }
    }
}

#[derive(Debug, Clone)]
struct ReleasedResource {
    resource_id: NodeId,
    resource_parent: Option<NodeId>,
    resource_index: usize,
    resource_node: SvgNode,
    moved_children: Vec<NodeId>,
}

/// Clears clip path or mask references from selected targets, restoring unused
/// resource contents to the canvas.
#[derive(Debug)]
pub struct ReleaseClipMaskCommand {
    mode: ClipMaskMode,
    target_ids: Vec<NodeId>,
    old_targets: Vec<TargetState>,
    released_resources: Vec<ReleasedResource>,
}

impl ReleaseClipMaskCommand {
    pub fn new(mode: ClipMaskMode, target_ids: Vec<NodeId>) -> Self {
        Self {
            mode,
            target_ids,
            old_targets: Vec::new(),
            released_resources: Vec::new(),
        }
    }

    fn href_target_id(reference: &str) -> Option<String> {
        let reference = reference.trim();
        if let Some(id) = reference.strip_prefix('#') {
            return (!id.is_empty()).then(|| id.to_string());
        }
        let inner = reference.strip_prefix("url(")?.strip_suffix(')')?.trim();
        let inner = inner.trim_matches('"').trim_matches('\'');
        inner
            .strip_prefix('#')
            .filter(|id| !id.is_empty())
            .map(|id| id.to_string())
    }

    fn reference_matches(reference: &str, xml_id: &str) -> bool {
        Self::href_target_id(reference).as_deref() == Some(xml_id)
    }

    fn resource_still_referenced(doc: &SvgDocument, mode: ClipMaskMode, xml_id: &str) -> bool {
        doc.nodes().any(|(_, node)| {
            mode.presentation_ref(&node.presentation)
                .map(|reference| Self::reference_matches(reference, xml_id))
                .unwrap_or(false)
                || node
                    .attributes
                    .get(mode.attr_name())
                    .map(|reference| Self::reference_matches(reference, xml_id))
                    .unwrap_or(false)
        })
    }

    fn release_resource_if_unused(
        &mut self,
        doc: &mut SvgDocument,
        resource_id: NodeId,
        xml_id: &str,
    ) {
        if Self::resource_still_referenced(doc, self.mode, xml_id) {
            return;
        }

        let Some(resource_node) = doc.get_node(resource_id).cloned() else {
            return;
        };
        let resource_parent = doc.parent(resource_id);
        let resource_index = doc.child_index(resource_id).unwrap_or(0);
        let moved_children = resource_node.children.clone();
        let restore_parent = doc.root();

        if let Some(parent) = restore_parent {
            for child_id in &moved_children {
                doc.detach_node(*child_id);
                doc.append_child(parent, *child_id);
            }
        }

        doc.detach_node(resource_id);
        doc.remove_node(resource_id);
        self.released_resources.push(ReleasedResource {
            resource_id,
            resource_parent,
            resource_index,
            resource_node,
            moved_children,
        });
    }
}

impl Command for ReleaseClipMaskCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        self.released_resources.clear();
        if self.old_targets.is_empty() {
            self.old_targets = self
                .target_ids
                .iter()
                .filter_map(|&id| TargetState::capture(doc, id))
                .collect();
        }

        let mut candidate_resources = Vec::new();
        for &target_id in &self.target_ids {
            let Some(node) = doc.get_node(target_id) else {
                continue;
            };
            let reference = self
                .mode
                .presentation_ref(&node.presentation)
                .map(|value| value.to_string())
                .or_else(|| node.attributes.get(self.mode.attr_name()).cloned());
            let Some(reference) = reference else {
                continue;
            };
            let Some(xml_id) = Self::href_target_id(&reference) else {
                continue;
            };
            let Some(resource_id) = doc.find_by_xml_id(&xml_id) else {
                continue;
            };
            candidate_resources.push((resource_id, xml_id));
        }

        for &target_id in &self.target_ids {
            if let Some(node) = doc.get_node_mut(target_id) {
                self.mode.set_presentation_ref(&mut node.presentation, None);
                node.attributes.remove(self.mode.attr_name());
                doc.notify_attribute_changed(target_id, self.mode.attr_name());
            }
        }

        candidate_resources.sort_by_key(|(id, _)| id.to_string());
        candidate_resources.dedup_by_key(|(id, _)| *id);
        for (resource_id, xml_id) in candidate_resources {
            self.release_resource_if_unused(doc, resource_id, &xml_id);
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        for released in self.released_resources.iter().rev() {
            let mut resource_node = released.resource_node.clone();
            resource_node.children.clear();
            resource_node.parent = None;
            doc.insert_node(resource_node);
            if let Some(parent) = released.resource_parent {
                doc.insert_child_at(parent, released.resource_id, released.resource_index);
            }
            for child_id in &released.moved_children {
                doc.detach_node(*child_id);
                doc.append_child(released.resource_id, *child_id);
            }
        }

        for target in &self.old_targets {
            target.restore(doc);
        }
    }

    fn description(&self) -> &str {
        match self.mode {
            ClipMaskMode::Clip => "Release clip",
            ClipMaskMode::Mask => "Release mask",
        }
    }
}
