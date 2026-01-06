use std::collections::HashSet;

use crate::error::SpecmanError;

use super::index::{ConstraintIdentifier, HeadingIdentifier, WorkspaceIndex};

pub trait StructureQuery {
    fn list_heading_slugs(&self) -> Vec<HeadingIdentifier>;
    fn list_constraint_groups(&self) -> Vec<ConstraintIdentifier>;

    fn render_heading(&self, heading: &HeadingIdentifier) -> Result<String, SpecmanError>;
    fn render_heading_by_slug(&self, slug: &str) -> Result<String, SpecmanError>;

    fn render_constraint_group(&self, group: &ConstraintIdentifier)
    -> Result<String, SpecmanError>;
}

impl StructureQuery for WorkspaceIndex {
    fn list_heading_slugs(&self) -> Vec<HeadingIdentifier> {
        let mut ids: Vec<_> = self.headings.keys().cloned().collect();
        ids.sort_by(|a, b| {
            let ao = self.headings.get(a).map(|h| h.order).unwrap_or(usize::MAX);
            let bo = self.headings.get(b).map(|h| h.order).unwrap_or(usize::MAX);
            (a.artifact.clone(), ao, a.slug.clone()).cmp(&(b.artifact.clone(), bo, b.slug.clone()))
        });
        ids
    }

    fn list_constraint_groups(&self) -> Vec<ConstraintIdentifier> {
        let mut ids: Vec<_> = self.constraints.keys().cloned().collect();
        ids.sort();
        ids
    }

    fn render_heading(&self, heading: &HeadingIdentifier) -> Result<String, SpecmanError> {
        self.render_heading_internal(heading, true)
    }

    fn render_heading_by_slug(&self, slug: &str) -> Result<String, SpecmanError> {
        let matches: Vec<_> = self
            .headings
            .keys()
            .filter(|id| id.slug == slug)
            .cloned()
            .collect();

        if matches.is_empty() {
            return Err(SpecmanError::Workspace(format!(
                "no heading slug '{slug}' found in workspace"
            )));
        }

        if matches.len() > 1 {
            let mut listed: Vec<String> = matches
                .iter()
                .map(|id| format!("{}#{}", id.artifact.workspace_path, id.slug))
                .collect();
            listed.sort();
            return Err(SpecmanError::Workspace(format!(
                "heading slug '{slug}' is ambiguous across artifacts: {}",
                listed.join(", ")
            )));
        }

        self.render_heading(&matches[0])
    }

    fn render_constraint_group(
        &self,
        group: &ConstraintIdentifier,
    ) -> Result<String, SpecmanError> {
        let record = self.constraints.get(group).ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "constraint group '{}' not found in {}",
                group.group, group.artifact.workspace_path
            ))
        })?;

        // Initial queue: Associated Heading -> Associated Heading references -> Constraint references
        let mut queue: Vec<HeadingIdentifier> = Vec::new();
        queue.push(record.heading.clone());

        if let Some(heading_record) = self.headings.get(&record.heading) {
            for link in &heading_record.referenced_headings {
                queue.push(link.clone());
            }
        }

        for link in &record.referenced_headings {
            queue.push(link.clone());
        }

        let mut rendered = String::new();
        let mut visited: HashSet<HeadingIdentifier> = HashSet::new();

        // Perform transitive closure via BFS/queue consumption.
        let mut i = 0;
        while i < queue.len() {
            let id = queue[i].clone();
            i += 1;

            if !visited.insert(id.clone()) {
                continue;
            }

            let section = self.render_heading_section(&id)?;
            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(&section);

            // Append transitive references
            if let Some(h) = self.headings.get(&id) {
                for r in &h.referenced_headings {
                    if !visited.contains(r) {
                        queue.push(r.clone());
                    }
                }
            }
        }

        Ok(rendered)
    }
}

impl WorkspaceIndex {
    fn render_heading_internal(
        &self,
        heading: &HeadingIdentifier,
        include_references: bool,
    ) -> Result<String, SpecmanError> {
        let mut rendered = String::new();

        let mut visited: HashSet<HeadingIdentifier> = HashSet::new();
        let mut queue: Vec<HeadingIdentifier> = Vec::new();

        queue.push(heading.clone());

        // Collect referenced headings in-link order, deduped.
        if include_references {
            let base = self.headings.get(heading).ok_or_else(|| {
                SpecmanError::Workspace(format!(
                    "heading '{}' not found in {}",
                    heading.slug, heading.artifact.workspace_path
                ))
            })?;

            for target in &base.referenced_headings {
                queue.push(target.clone());
            }
        }

        for id in queue {
            if !visited.insert(id.clone()) {
                continue;
            }
            let section = self.render_heading_section(&id)?;
            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(&section);
        }

        Ok(rendered)
    }

    fn render_heading_section(&self, heading: &HeadingIdentifier) -> Result<String, SpecmanError> {
        let record = self.headings.get(heading).ok_or_else(|| {
            SpecmanError::Workspace(format!(
                "heading '{}' not found in {}",
                heading.slug, heading.artifact.workspace_path
            ))
        })?;

        let mut out = String::new();
        out.push_str(&"#".repeat(record.level as usize));
        out.push(' ');
        out.push_str(&record.title);
        out.push('\n');

        if !record.content.is_empty() {
            out.push_str(&record.content);
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }

        // Render children (nesting) in document order.
        let mut children = record.children.clone();
        children.sort_by_key(|id| self.headings.get(id).map(|h| h.order).unwrap_or(usize::MAX));
        for child in children {
            out.push('\n');
            out.push_str(&self.render_heading_section(&child)?);
        }

        Ok(out)
    }
}
