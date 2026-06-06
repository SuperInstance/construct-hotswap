//! # construct-hotswap
//!
//! Tests live construct hotswap with CRDT state synchronization.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstructState { Loaded, Deployed, Draining, Cached }

#[derive(Debug, Clone)]
pub struct Construct {
    pub name: String,
    pub version: String,
    pub kernel_ptx: Vec<u8>,
    pub state: ConstructState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    Load, Deploy, CrdtSync, HotswapDrain, HotswapComplete, Unload,
}

#[derive(Debug, Clone)]
pub struct HotswapEvent {
    pub time_us: u64,
    pub event_type: EventType,
    pub node: String,
    pub construct: String,
}

struct NodeState {
    node_id: String,
    loaded: HashMap<String, String>,
    timestamp: u64,
}

pub struct HotswapExperiment {
    nodes: Vec<NodeState>,
    deployed: HashMap<String, Construct>,
    events: Vec<HotswapEvent>,
    time_us: u64,
}

impl HotswapExperiment {
    pub fn new(node_count: usize) -> Self {
        Self {
            nodes: (0..node_count).map(|i| NodeState {
                node_id: format!("gpu-{}", i), loaded: HashMap::new(), timestamp: 0,
            }).collect(),
            deployed: HashMap::new(), events: Vec::new(), time_us: 0,
        }
    }

    pub fn load(&mut self, name: &str, version: &str) {
        self.time_us += 100;
        self.deployed.insert(name.into(), Construct {
            name: name.into(), version: version.into(),
            kernel_ptx: vec![0x7f; 1024], state: ConstructState::Loaded,
        });
        self.events.push(HotswapEvent { time_us: self.time_us, event_type: EventType::Load, node: "ctrl".into(), construct: name.into() });
    }

    pub fn deploy(&mut self, name: &str, node_idx: usize) -> Result<(), String> {
        let c = self.deployed.get_mut(name).ok_or("not loaded")?;
        c.state = ConstructState::Deployed;
        self.nodes[node_idx].loaded.insert(name.into(), c.version.clone());
        self.nodes[node_idx].timestamp = self.time_us;
        self.time_us += 50;
        self.events.push(HotswapEvent { time_us: self.time_us, event_type: EventType::Deploy, node: self.nodes[node_idx].node_id.clone(), construct: name.into() });
        Ok(())
    }

    pub fn crdt_sync(&mut self) {
        self.time_us += 200;
        let mut global: HashMap<String, String> = HashMap::new();
        for n in &self.nodes { for (k, v) in &n.loaded { global.insert(k.clone(), v.clone()); } }
        for n in &mut self.nodes { for (k, v) in &global { n.loaded.insert(k.clone(), v.clone()); } n.timestamp = self.time_us; }
        self.events.push(HotswapEvent { time_us: self.time_us, event_type: EventType::CrdtSync, node: "all".into(), construct: "all".into() });
    }

    pub fn hotswap(&mut self, name: &str, new_version: &str) -> Result<u64, String> {
        let c = self.deployed.get_mut(name).ok_or("not deployed")?;
        c.state = ConstructState::Draining;
        self.events.push(HotswapEvent { time_us: self.time_us, event_type: EventType::HotswapDrain, node: "all".into(), construct: name.into() });
        self.time_us += 300;
        c.version = new_version.into();
        c.kernel_ptx = vec![0x7f; 2048];
        c.state = ConstructState::Deployed;
        self.time_us += 100;
        self.events.push(HotswapEvent { time_us: self.time_us, event_type: EventType::HotswapComplete, node: "all".into(), construct: name.into() });
        self.crdt_sync();
        Ok(self.time_us)
    }

    pub fn total_time_us(&self) -> u64 { self.time_us }
    pub fn events(&self) -> &[HotswapEvent] { &self.events }
    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn deployed_count(&self) -> usize {
        self.deployed.values().filter(|c| c.state == ConstructState::Deployed).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_deploy() {
        let mut e = HotswapExperiment::new(3);
        e.load("attention", "v1.0");
        assert_eq!(e.deployed_count(), 0);
        e.deploy("attention", 0).unwrap();
        assert_eq!(e.deployed_count(), 1);
    }

    #[test]
    fn test_crdt_sync() {
        let mut e = HotswapExperiment::new(3);
        e.load("reduce", "v1.0");
        e.deploy("reduce", 0).unwrap();
        e.crdt_sync();
        for n in &e.nodes { assert!(n.loaded.contains_key("reduce")); }
    }

    #[test]
    fn test_hotswap() {
        let mut e = HotswapExperiment::new(3);
        e.load("attention", "v1.0");
        e.deploy("attention", 0).unwrap();
        e.crdt_sync();
        e.hotswap("attention", "v2.0").unwrap();
        let c = e.deployed.get("attention").unwrap();
        assert_eq!(c.version, "v2.0");
        assert_eq!(c.state, ConstructState::Deployed);
    }

    #[test]
    fn test_hotswap_events() {
        let mut e = HotswapExperiment::new(2);
        e.load("filter", "v1.0");
        e.deploy("filter", 0).unwrap();
        e.hotswap("filter", "v2.0").unwrap();
        let types: Vec<_> = e.events().iter().map(|ev| &ev.event_type).collect();
        assert!(types.contains(&&EventType::HotswapDrain));
        assert!(types.contains(&&EventType::HotswapComplete));
    }

    #[test]
    fn test_multi_construct() {
        let mut e = HotswapExperiment::new(4);
        e.load("a", "v1"); e.load("b", "v1");
        e.deploy("a", 0).unwrap(); e.deploy("b", 1).unwrap();
        e.crdt_sync();
        e.hotswap("a", "v2").unwrap();
        assert_eq!(e.deployed_count(), 2);
    }

    #[test]
    fn test_deploy_nonexistent() {
        let mut e = HotswapExperiment::new(2);
        assert!(e.deploy("nope", 0).is_err());
    }
}
