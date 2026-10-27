//! The requirement levels as a tree (FORMAT.md, "Requirement levels"):
//! each top-level requirement with the requirements of the level below
//! that trace to it, down to the lowest level, plus the derived
//! requirements and those that trace to nothing. `kiln trace --levels`
//! and the `levels` report both read this, so they agree.
//!
//! A requirement traces through its parent when it has no trace link of
//! its own, as `kiln check`'s K208 counts it: decomposing within a level
//! puts the children beside their parent, under the requirement above.
//! It doesn't judge: a link that skips a level (K209) still places the
//! item under its target, at the item's own level.

use std::collections::{HashMap, HashSet};

use kiln_model::{Item, ItemId};

use crate::project::Project;

/// One requirement and the requirements of the level below tracing to
/// it.
#[derive(Debug, Clone)]
pub struct LevelNode<'a> {
    pub item: &'a Item,
    /// 0 for the highest level.
    pub level: usize,
    pub below: Vec<LevelNode<'a>>,
}

/// Every open requirement of every level, placed.
#[derive(Debug, Clone)]
pub struct LevelTrace<'a> {
    /// The level type names, highest first.
    pub levels: Vec<&'a str>,
    /// The highest level's requirements, in item order.
    pub roots: Vec<LevelNode<'a>>,
    /// Requirements below the top level marked `derived` (or under a
    /// derived parent), with what traces to them.
    pub derived: Vec<LevelNode<'a>>,
    /// Requirements below the top level that trace to nothing open and
    /// aren't derived.
    pub untraced: Vec<LevelNode<'a>>,
}

impl LevelNode<'_> {
    /// Every path from this node down to a node with nothing below, as
    /// the items along it: the rows of a traceability matrix.
    pub fn paths(&self) -> Vec<Vec<&Item>> {
        if self.below.is_empty() {
            return vec![vec![self.item]];
        }
        self.below
            .iter()
            .flat_map(LevelNode::paths)
            .map(|mut path| {
                path.insert(0, self.item);
                path
            })
            .collect()
    }
}

/// Where a requirement traces to.
enum Up {
    /// The highest level, or no trace role: a root.
    Top,
    /// The open items it (or the ancestor it traces through) points at.
    To(Vec<ItemId>),
    Derived,
    Nowhere,
}

impl Project {
    /// The requirement levels as a tree. With one level (or none) every
    /// requirement is a root.
    // implements: REQ-018
    pub fn level_trace(&self) -> LevelTrace<'_> {
        let config = &self.config;
        let levels = config.levels();
        let trace = config.trace().map(|r| r.name.as_str());
        let mut seen_ids = HashSet::new();
        let open: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| config.level_of(&i.type_name).is_some() && !self.is_closed(i))
            .filter(|i| seen_ids.insert(i.id.clone()))
            .collect();

        // Each item's way up, once; then who traces to whom.
        let mut below: HashMap<&ItemId, Vec<&Item>> = HashMap::new();
        let mut ups = Vec::with_capacity(open.len());
        for &item in &open {
            let level = config.level_of(&item.type_name).unwrap_or(0);
            let up = match trace {
                Some(trace) if level > 0 => self.up(item, trace),
                _ => Up::Top,
            };
            let mut placed = false;
            if let Up::To(targets) = &up {
                for target in targets {
                    // `up` keeps only open targets; a target of no level
                    // or of the same level or lower isn't "above".
                    if let Some(t) = self.item(target) {
                        if config.level_of(&t.type_name).is_some_and(|l| l < level) {
                            let under = below.entry(&t.id).or_default();
                            if !under.iter().any(|i| i.id == item.id) {
                                under.push(item);
                            }
                            placed = true;
                        }
                    }
                }
            }
            // Tracing only sideways or down places it nowhere: untraced.
            let up = match up {
                Up::To(_) if !placed => Up::Nowhere,
                up => up,
            };
            ups.push((item, level, up));
        }

        let mut out = LevelTrace {
            levels,
            roots: Vec::new(),
            derived: Vec::new(),
            untraced: Vec::new(),
        };
        for (item, level, up) in ups {
            let node = || node(item, level, &below, config, &mut HashSet::new());
            match up {
                Up::Top => out.roots.push(node()),
                Up::To(_) => {}
                Up::Derived => out.derived.push(node()),
                Up::Nowhere => out.untraced.push(node()),
            }
        }
        out
    }

    /// What `item` traces to: its own open trace targets, or else those
    /// of the nearest ancestor of its type that has any; derived when it
    /// or that ancestor says so first.
    fn up(&self, item: &Item, trace: &str) -> Up {
        let mut seen = HashSet::new();
        let mut at = Some(item);
        while let Some(i) = at {
            if i.type_name != item.type_name || !seen.insert(i.id.clone()) {
                break;
            }
            if i.derived.is_some() {
                return Up::Derived;
            }
            let to: Vec<ItemId> = i
                .links
                .iter()
                .filter(|l| l.role == trace)
                .filter_map(|l| l.target.id.as_ref().ok())
                .filter(|id| self.item(id).is_some_and(|t| !self.is_closed(t)))
                .cloned()
                .collect();
            if !to.is_empty() {
                return Up::To(to);
            }
            at = i
                .parent
                .as_ref()
                .and_then(|p| p.id.as_ref().ok())
                .and_then(|id| self.item(id));
        }
        Up::Nowhere
    }
}

/// `item`'s node, with everything tracing to it, recursively; an item
/// already on the path (a trace cycle) is left out rather than looped.
fn node<'a>(
    item: &'a Item,
    level: usize,
    below: &HashMap<&ItemId, Vec<&'a Item>>,
    config: &kiln_model::Config,
    path: &mut HashSet<ItemId>,
) -> LevelNode<'a> {
    let mut nodes = Vec::new();
    if path.insert(item.id.clone()) {
        for &other in below.get(&item.id).map(Vec::as_slice).unwrap_or(&[]) {
            if !path.contains(&other.id) {
                let other_level = config.level_of(&other.type_name).unwrap_or(0);
                nodes.push(node(other, other_level, below, config, path));
            }
        }
        path.remove(&item.id);
    }
    LevelNode {
        item,
        level,
        below: nodes,
    }
}
