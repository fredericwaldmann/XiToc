//! Scoped object arrangement through the existing transform and link journal.
use super::*;
impl Drawing {
    /// Derived connector paths do not determine rectangular alignment bounds,
    /// including inside a group with an externally anchored wire.
    pub fn alignment_bounds(&self, id: Uuid) -> Option<crate::object::ObjectBounds> {
        match self.snapshot(id)? {
            ObjectSnapshot::Connector(_) => None,
            ObjectSnapshot::Group(g) => g
                .children
                .iter()
                .filter_map(|id| self.alignment_bounds(*id))
                .reduce(crate::object::ObjectBounds::union),
            object => object.as_object().world_bounds(),
        }
    }

    pub fn alignment_targets(
        &self,
        ids: &std::collections::HashSet<Uuid>,
    ) -> Vec<crate::snapping::Target> {
        self.items
            .iter()
            .filter(|o| {
                ids.contains(&o.id())
                    && self.in_scope(o.id())
                    && !matches!(o, ObjectSnapshot::Connector(_))
            })
            .filter(|o| {
                self.expanded(&std::collections::HashSet::from([o.id()]))
                    .iter()
                    .all(|id| !self.is_locked(*id))
            })
            .filter_map(|o| {
                Some(crate::snapping::Target {
                    id: o.id(),
                    bounds: self.alignment_bounds(o.id())?,
                })
            })
            .collect()
    }
    pub fn snap_targets(
        &self,
        surface: Surface,
        excluded: &std::collections::HashSet<Uuid>,
    ) -> Vec<crate::snapping::Target> {
        self.items
            .iter()
            .filter(|o| {
                self.parent(o.id()) == self.editing_group
                    && o.as_object().surface() == surface
                    && !excluded.contains(&o.id())
                    && !matches!(o, ObjectSnapshot::Connector(_))
            })
            .filter_map(|o| {
                Some(crate::snapping::Target {
                    id: o.id(),
                    bounds: self.alignment_bounds(o.id())?,
                })
            })
            .collect()
    }
    pub fn align_objects(
        &mut self,
        ids: &std::collections::HashSet<Uuid>,
        command: crate::snapping::Align,
    ) -> Result<bool, &'static str> {
        let targets = self.alignment_targets(ids);
        let movements = crate::snapping::alignment(&targets, command)?;
        let mut placements = std::collections::HashMap::new();
        for (id, movement) in movements {
            if movement == [0.; 2] {
                continue;
            }
            let delta = crate::object::Placement {
                position: movement,
                ..Default::default()
            };
            for descendant in self.expanded(&std::collections::HashSet::from([id])) {
                if placements.insert(descendant, delta).is_some() {
                    return Err("Cannot align overlapping ownership scopes.");
                }
            }
        }
        if placements.is_empty() {
            return Ok(false);
        }
        let selected = placements.keys().copied().collect();
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (index, source) in self.items.iter().enumerate() {
            let Some(delta) = placements.get(&source.id()) else {
                continue;
            };
            if self.is_locked(source.id()) {
                return Err("A descendant is locked.");
            }
            let target = match source {
                ObjectSnapshot::Connector(c) => c
                    .transformed_in_group(
                        *delta,
                        &selected,
                        self.parent(c.id).is_some_and(|p| selected.contains(&p)),
                    )
                    .into(),
                _ => source.transformed(*delta),
            };
            if !self.valid_object(&target) {
                return Err("Alignment would leave this page or exceed supported coordinates.");
            }
            before.push((index, source.clone()));
            after.push((index, target));
        }
        if !self.update_links(&mut before, &mut after) {
            return Err("Cannot update attached connectors for this alignment.");
        }
        // Keep the entire original selection, including pinned distribution ends.
        self.execute_selected(
            Command::Replace { before, after },
            Some(ids.iter().copied().collect()),
        );
        Ok(true)
    }
}
