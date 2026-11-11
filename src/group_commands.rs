use super::*;
// Included in drawing.rs: organization uses the same canonical store and journal.
impl Drawing {
    pub fn parent(&self, id: Uuid) -> Option<Uuid> {
        self.parents.get(&id).copied()
    }
    pub fn is_locked(&self, mut id: Uuid) -> bool {
        loop {
            if self.snapshot(id).is_some_and(ObjectSnapshot::locked) {
                return true;
            }
            let Some(parent) = self.parent(id) else {
                return false;
            };
            id = parent;
        }
    }
    pub fn in_scope(&self, id: Uuid) -> bool {
        self.snapshot(id).is_some() && self.parent(id) == self.editing_group && !self.is_locked(id)
    }
    pub fn scoped_target(&self, mut id: Uuid) -> Option<Uuid> {
        if self.is_locked(id) {
            return None;
        }
        loop {
            let parent = self.parent(id);
            if parent == self.editing_group {
                return Some(id);
            }
            id = parent?;
        }
    }
    pub fn expanded(
        &self,
        ids: &std::collections::HashSet<Uuid>,
    ) -> std::collections::HashSet<Uuid> {
        let mut result = ids.clone();
        let mut pending: Vec<_> = ids.iter().copied().collect();
        while let Some(id) = pending.pop() {
            if let Some(ObjectSnapshot::Group(g)) = self.snapshot(id) {
                for child in &g.children {
                    if result.insert(*child) {
                        pending.push(*child);
                    }
                }
            }
        }
        result
    }
    pub fn enter_group(&mut self, id: Uuid) -> bool {
        if self.in_scope(id) && matches!(self.snapshot(id), Some(ObjectSnapshot::Group(_))) {
            self.editing_group = Some(id);
            true
        } else {
            false
        }
    }
    pub fn exit_group(&mut self) -> Option<Uuid> {
        let id = self.editing_group?;
        self.editing_group = self.parent(id);
        Some(id)
    }
    pub fn group_selection(&mut self, ids: &std::collections::HashSet<Uuid>) -> Option<Uuid> {
        if ids.len() < 2 || ids.iter().any(|id| !self.in_scope(*id)) {
            return None;
        }
        let children: Vec<_> = self
            .items
            .iter()
            .filter(|o| ids.contains(&o.id()))
            .map(ObjectSnapshot::id)
            .collect();
        let surface = self.snapshot(children[0])?.as_object().surface();
        if children
            .iter()
            .any(|id| self.snapshot(*id).unwrap().as_object().surface() != surface)
        {
            return None;
        }
        let id = Uuid::new_v4();
        let group = crate::group::Group {
            id,
            document_id: self.owner,
            surface,
            children,
            locked: false,
            bounds: None,
        };
        let mut before = Vec::new();
        let index = if self.editing_group.is_some() {
            self.items.len()
        } else {
            self.items.iter().rposition(|o| ids.contains(&o.id()))? + 1
        };
        let mut after = vec![(index, ObjectSnapshot::Group(Arc::new(group)))];
        if let Some(parent) = self.editing_group {
            let index = self.object_slots[&parent];
            let ObjectSnapshot::Group(g) = &self.items[index] else {
                return None;
            };
            let mut g = (**g).clone();
            let last = g.children.iter().rposition(|id| ids.contains(id))?;
            let mut children = Vec::new();
            for (i, child) in g.children.iter().enumerate() {
                if !ids.contains(child) {
                    children.push(*child);
                }
                if i == last {
                    children.push(id);
                }
            }
            g.children = children;
            before.push((index, self.items[index].clone()));
            // Append the new container before replacing its parent during normalization.
            after.insert(0, (index, ObjectSnapshot::Group(Arc::new(g))));
        }
        after.sort_by_key(|(i, _)| *i);
        let changed: std::collections::HashSet<_> = before.iter().map(|(_, o)| o.id()).collect();
        let prospective: Vec<_> = self
            .items
            .iter()
            .filter(|o| !changed.contains(&o.id()))
            .cloned()
            .chain(after.iter().map(|(_, o)| o.clone()))
            .collect();
        if !crate::group::validate(&prospective) {
            return None;
        }
        self.execute_selected(Command::Replace { before, after }, Some(vec![id]));
        self.history.last_mut().unwrap().undo_selection = Some(ids.iter().copied().collect());
        Some(id)
    }
    pub fn ungroup_selection(&mut self, ids: &std::collections::HashSet<Uuid>) -> Vec<Uuid> {
        let groups: Vec<_> = ids
            .iter()
            .filter_map(|id| match self.snapshot(*id) {
                Some(ObjectSnapshot::Group(g)) if self.in_scope(*id) => Some(g.clone()),
                _ => None,
            })
            .collect();
        if groups.is_empty() {
            return Vec::new();
        }
        let mut next = self.items.clone();
        let mut selected = Vec::new();
        for g in groups {
            selected.extend(g.children.iter().copied());
            next.retain(|o| o.id() != g.id);
            for object in &mut next {
                if let ObjectSnapshot::Group(parent) = object
                    && parent.children.contains(&g.id)
                {
                    let parent = Arc::make_mut(parent);
                    let index = parent.children.iter().position(|id| *id == g.id).unwrap();
                    parent
                        .children
                        .splice(index..=index, g.children.iter().copied());
                }
            }
        }
        self.replace_organization(next, selected.clone());
        self.history.last_mut().unwrap().undo_selection = Some(ids.iter().copied().collect());
        selected
    }
    pub fn lock_objects(&mut self, ids: &std::collections::HashSet<Uuid>, locked: bool) -> bool {
        let mut before = Vec::new();
        let mut after = Vec::new();
        for id in ids {
            let Some(object) = self.snapshot(*id) else {
                continue;
            };
            // Unlock a directly targeted container, never clear descendant locks.
            if object.locked() == locked || self.parent(*id).is_some_and(|p| self.is_locked(p)) {
                continue;
            }
            let index = self.object_slots[id];
            let mut new = object.clone();
            new.set_locked(locked);
            before.push((index, object.clone()));
            after.push((index, new));
        }
        if before.is_empty() {
            return false;
        }
        before.sort_by_key(|(i, _)| *i);
        after.sort_by_key(|(i, _)| *i);
        self.execute_selected(
            Command::Replace { before, after },
            Some(ids.iter().copied().collect()),
        );
        true
    }
    pub fn arrange(
        &mut self,
        ids: &std::collections::HashSet<Uuid>,
        command: crate::selection::SelectionCommand,
    ) -> bool {
        use crate::selection::SelectionCommand::*;
        if ids.is_empty() || ids.iter().any(|id| !self.in_scope(*id)) {
            return false;
        }
        let mut order: Vec<_> = self
            .items
            .iter()
            .filter(|o| self.parent(o.id()) == self.editing_group)
            .map(ObjectSnapshot::id)
            .collect();
        let original = order.clone();
        match command {
            BringToFront => order.sort_by_key(|id| ids.contains(id)),
            SendToBack => order.sort_by_key(|id| !ids.contains(id)),
            BringForward => {
                for i in (0..order.len().saturating_sub(1)).rev() {
                    if ids.contains(&order[i]) && !ids.contains(&order[i + 1]) {
                        order.swap(i, i + 1);
                    }
                }
            }
            SendBackward => {
                for i in 1..order.len() {
                    if ids.contains(&order[i]) && !ids.contains(&order[i - 1]) {
                        order.swap(i, i - 1);
                    }
                }
            }
            _ => return false,
        }
        if original == order {
            return false;
        }
        let mut next = self.items.clone();
        if let Some(id) = self.editing_group {
            let ObjectSnapshot::Group(g) = &mut next[self.object_slots[&id]] else {
                return false;
            };
            Arc::make_mut(g).children = order;
        } else {
            // Root order is the order of root records; children are flattened below.
            let roots: std::collections::HashSet<_> = order.iter().copied().collect();
            next.retain(|o| !roots.contains(&o.id()));
            next.extend(order.iter().map(|id| self.snapshot(*id).unwrap().clone()));
        }
        self.replace_organization(next, ids.iter().copied().collect());
        true
    }
    fn replace_organization(&mut self, next: Vec<ObjectSnapshot>, selected: Vec<Uuid>) {
        let before = self.items.iter().cloned().enumerate().collect();
        let after = next.into_iter().enumerate().collect();
        self.execute_selected(Command::Replace { before, after }, Some(selected));
    }
    /// Postorder storage means each group occupies one contiguous stacking block.
    /// Geometry remains world-space; containers own no second transform.
    pub(super) fn normalize_groups(&mut self) {
        let live: std::collections::HashSet<_> =
            self.items.iter().map(ObjectSnapshot::id).collect();
        for object in &mut self.items {
            if let ObjectSnapshot::Group(g) = object
                && g.children.iter().any(|id| !live.contains(id))
            {
                Arc::make_mut(g).children.retain(|id| live.contains(id));
            }
        }
        // Empty containers are removed, including newly empty ancestors.
        loop {
            let empty: std::collections::HashSet<_> = self
                .items
                .iter()
                .filter_map(|o| match o {
                    ObjectSnapshot::Group(g) if g.children.is_empty() => Some(g.id),
                    _ => None,
                })
                .collect();
            if empty.is_empty() {
                break;
            }
            self.items.retain(|o| !empty.contains(&o.id()));
            for object in &mut self.items {
                if let ObjectSnapshot::Group(g) = object
                    && g.children.iter().any(|id| empty.contains(id))
                {
                    Arc::make_mut(g).children.retain(|id| !empty.contains(id));
                }
            }
        }
        let slots: std::collections::HashMap<_, _> =
            self.items.iter().map(|o| (o.id(), o.clone())).collect();
        let children: std::collections::HashSet<_> = self
            .items
            .iter()
            .filter_map(|o| match o {
                ObjectSnapshot::Group(g) => Some(&g.children),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        fn append(
            id: Uuid,
            slots: &std::collections::HashMap<Uuid, ObjectSnapshot>,
            result: &mut Vec<ObjectSnapshot>,
        ) -> Option<crate::object::ObjectBounds> {
            let object = slots.get(&id)?;
            if let ObjectSnapshot::Group(g) = object {
                let bounds = g
                    .children
                    .iter()
                    .filter_map(|id| append(*id, slots, result))
                    .reduce(crate::object::ObjectBounds::union);
                let mut g = (**g).clone();
                g.bounds = bounds;
                result.push(ObjectSnapshot::Group(Arc::new(g)));
                bounds
            } else {
                result.push(object.clone());
                object.as_object().world_bounds()
            }
        }
        let mut result = Vec::with_capacity(self.items.len());
        for object in &self.items {
            if !children.contains(&object.id()) {
                append(object.id(), &slots, &mut result);
            }
        }
        self.items = result;
    }
}
impl Drawing {
    pub fn area_targets(&self, hits: &[Uuid], crossing: bool) -> Vec<Uuid> {
        let hits: std::collections::HashSet<_> = hits.iter().copied().collect();
        let candidates: std::collections::HashSet<_> = hits
            .iter()
            .filter_map(|id| self.scoped_target(*id))
            .collect();
        candidates
            .into_iter()
            .filter(|id| {
                if crossing {
                    return true;
                }
                self.expanded(&std::collections::HashSet::from([*id]))
                    .iter()
                    .all(|id| {
                        matches!(self.snapshot(*id), Some(ObjectSnapshot::Group(_)))
                            || hits.contains(id)
                    })
            })
            .collect()
    }
}
