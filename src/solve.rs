use crate::activity::Activities;
use crate::instance::{Instance, NodeIdx};
use anyhow::Result;
use log::{debug, info, trace};
use rand::{Rng, SeedableRng};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
struct Stats {
    iterations: usize,
    node_deletions: usize,
    edge_deletions: usize,
}

#[derive(Debug, Clone)]
struct State<R: Rng> {
    rng: R,
    incomplete_hs: Vec<NodeIdx>,
    best_known: usize,
    activities: Activities<R>,
    stats: Stats,
}

fn greedy_approx(instance: &mut Instance) -> usize {
    let time_start = Instant::now();
    let mut hs = vec![];
    while !instance.edges().is_empty() {
        let mut max_degree = (0, NodeIdx::INVALID);
        for &node in instance.nodes() {
            max_degree = max_degree.max((instance.node_degree(node), node));
        }
        instance.delete_node(max_degree.1);
        instance.delete_incident_edges(max_degree.1);
        hs.push(max_degree.1);
    }
    for &node in hs.iter().rev() {
        instance.restore_incident_edges(node);
        instance.restore_node(node);
    }
    debug!(
        "Greedy hs of size {}: {:?} ({:.2?})",
        hs.len(),
        hs,
        Instant::now() - time_start
    );
    hs.len()
}

fn solve_recursive(instance: &mut Instance, state: &mut State<impl Rng>) {
    if instance.edges().is_empty() {
        debug!(
            "Found HS of size {}: {:?}",
            state.incomplete_hs.len(),
            state.incomplete_hs
        );
        state.best_known = state.incomplete_hs.len();
    }
    // Don't count the last iteration where we find a new best HS, since they
    // are comparatively very cheap
    state.stats.iterations += 1;

    if state.incomplete_hs.len() + 1 >= state.best_known || instance.nodes().is_empty() {
        for &node in &state.incomplete_hs {
            state.activities.boost_activity(node, 1.0);
        }
        return;
    }

    let node = state.activities.highest();
    state.stats.node_deletions += 1;
    state.stats.edge_deletions += instance.node_degree(node);

    trace!("Branching on {}", node);
    if state.rng.gen() {
        instance.delete_node(node);
        state.activities.delete(node);
        solve_recursive(instance, state);
        instance.delete_incident_edges(node);
        state.incomplete_hs.push(node);
        solve_recursive(instance, state);
        state.incomplete_hs.pop();
        instance.restore_incident_edges(node);
        instance.restore_node(node);
        state.activities.restore(node);
    } else {
        instance.delete_node(node);
        state.activities.delete(node);
        instance.delete_incident_edges(node);
        state.incomplete_hs.push(node);
        solve_recursive(instance, state);
        state.incomplete_hs.pop();
        instance.restore_incident_edges(node);
        solve_recursive(instance, state);
        instance.restore_node(node);
        state.activities.restore(node);
    }

    state.activities.decay_all();
}

pub fn solve(instance: &mut Instance, mut rng: impl Rng + SeedableRng) -> Result<usize> {
    let approx = greedy_approx(instance);
    let activities = Activities::new(instance, &mut rng)?;
    let mut state = State {
        rng,
        incomplete_hs: vec![],
        best_known: approx,
        activities,
        stats: Stats::default(),
    };
    let time_start = Instant::now();
    solve_recursive(instance, &mut state);
    info!(
        "Recursive solving took {} iterations ({:.2?})",
        state.stats.iterations,
        Instant::now() - time_start
    );
    info!(
        "Deleted/restored {} nodes and {} edges while solving",
        state.stats.node_deletions, state.stats.edge_deletions
    );
    Ok(state.best_known)
}
