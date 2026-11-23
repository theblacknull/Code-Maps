//! ua-viewer: trình xem knowledge-graph.json bằng Rust + egui, bám theo dashboard gốc của
//! Understand-Anything (packages/dashboard).
//!
//! Cấp xem:
//! - Tổng quan: mỗi layer là thẻ lớn (LayerClusterNode), cạnh giữa các layer gộp lại (edgeAggregation.ts).
//! - Chi tiết layer: node của layer, bố cục force theo KnowledgeGraphView.tsx, kèm cổng nối (PortalNode)
//!   sang các layer liên quan. Esc hoặc "← Tổng quan" để quay lại.
//!
//! Dùng: ua-viewer [đường_dẫn/knowledge-graph.json]   (mặc định .ua/knowledge-graph.json)
//! Phím: F fit, Esc quay lại, / tìm kiếm, ← → bước tour, H phím tắt, M minimap.

mod layout;

use eframe::egui::{self, Align2, Color32, FontId, Id, Pos2, Rect, Sense, Stroke, Vec2};
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

// ---------------------------------------------------------------- dữ liệu

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
struct Node {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    name: String,
    summary: String,
    #[serde(rename = "filePath")]
    file_path: Option<String>,
    tags: Vec<String>,
    complexity: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
struct Edge {
    source: String,
    target: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
struct Layer {
    id: String,
    name: String,
    description: String,
    #[serde(rename = "nodeIds")]
    node_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
struct Step {
    order: u32,
    title: String,
    description: String,
    #[serde(rename = "nodeIds")]
    node_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    layers: Vec<Layer>,
    tour: Vec<Step>,
}

/// Overlay thay đổi (DiffToggle): .ua/diff-overlay.json
#[derive(Debug, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DiffOverlay {
    changed_node_ids: Vec<String>,
    affected_node_ids: Vec<String>,
}

// ---------------------------------------------------------------- theme (themes/presets.ts)

#[derive(Clone, Copy)]
struct Pal {
    bg: Color32,
    surface: Color32,
    ink: Color32,
    muted: Color32,
    edge: Color32,
    border: Color32,
    hilite: Color32,
    accent: Color32,
    diff_changed: Color32,
    diff_affected: Color32,
}

fn hex(s: &str) -> Color32 {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

struct Preset {
    id: &'static str,
    name: &'static str,
    is_dark: bool,
    root: &'static str,
    surface: &'static str,
    text: &'static str,
    muted: &'static str,
    /// (tên, accent) — swatch của preset
    accents: &'static [(&'static str, &'static str)],
}

const DARK_ACCENTS: &[(&str, &str)] = &[
    ("Gold", "#d4a574"),
    ("Ocean", "#5ba4cf"),
    ("Emerald", "#5ea67a"),
    ("Rose", "#cf7a8a"),
    ("Purple", "#9b7abf"),
    ("Amber", "#c9963a"),
    ("Teal", "#4aab9a"),
    ("Silver", "#a0a8b0"),
];
const LIGHT_ACCENTS: &[(&str, &str)] = &[
    ("Indigo", "#4a6fa5"),
    ("Ocean", "#3a8ab5"),
    ("Emerald", "#3a8a5c"),
    ("Rose", "#a5566a"),
    ("Purple", "#6b5a9e"),
    ("Amber", "#9e7a30"),
    ("Teal", "#2e8a7a"),
    ("Slate", "#5a6570"),
];
const PLAIN_ACCENTS: &[(&str, &str)] = &[("Black", "#000000"), ("Gray", "#444444")];

const PRESETS: [Preset; 5] = [
    Preset { id: "light-plain", name: "Plain White", is_dark: false, root: "#ffffff", surface: "#ffffff", text: "#111111", muted: "#5b6472", accents: PLAIN_ACCENTS },
    Preset { id: "dark-gold", name: "Dark Gold", is_dark: true, root: "#0a0a0a", surface: "#111111", text: "#f5f0eb", muted: "#a39787", accents: DARK_ACCENTS },
    Preset { id: "dark-ocean", name: "Dark Ocean", is_dark: true, root: "#0a0e14", surface: "#111820", text: "#e8edf2", muted: "#87939f", accents: DARK_ACCENTS },
    Preset { id: "dark-forest", name: "Dark Forest", is_dark: true, root: "#0a0f0c", surface: "#111a14", text: "#e9f2ec", muted: "#8fa597", accents: DARK_ACCENTS },
    Preset { id: "light-minimal", name: "Light Minimal", is_dark: false, root: "#f5f3f0", surface: "#ffffff", text: "#1c1c1c", muted: "#6b6b6b", accents: LIGHT_ACCENTS },
];

fn palette(preset: &Preset, accent_idx: usize) -> Pal {
    let (_, acc) = preset.accents[accent_idx.min(preset.accents.len() - 1)];
    let muted = hex(preset.muted);
    Pal {
        bg: hex(preset.root),
        surface: hex(preset.surface),
        ink: hex(preset.text),
        muted,
        edge: if preset.is_dark { Color32::from_rgb(90, 90, 90) } else { Color32::from_rgb(203, 213, 225) },
        border: if preset.is_dark { Color32::from_rgb(60, 60, 60) } else { Color32::from_rgb(226, 232, 240) },
        hilite: if preset.is_dark { Color32::from_rgb(70, 60, 30) } else { Color32::from_rgb(254, 243, 199) },
        accent: hex(acc),
        diff_changed: Color32::from_rgb(224, 82, 82),
        diff_affected: Color32::from_rgb(212, 160, 48),
    }
}

/// Bảng màu nhận diện cho layer và loại node. Các tông đậm để chữ đạt tương phản ≥ 4.5:1 trên nền sáng.
const LAYER_COLORS: [&str; 8] = ["#1d4ed8", "#047857", "#6d28d9", "#b45309", "#be185d", "#0f766e", "#4338ca", "#c2410c"];

/// Trộn hai màu: t = 0 cho `base`, t = 1 cho `over`.
fn mix(base: Color32, over: Color32, t: f32) -> Color32 {
    let ch = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).round() as u8;
    Color32::from_rgb(ch(base.r(), over.r()), ch(base.g(), over.g()), ch(base.b(), over.b()))
}

/// Màu của một loại node, lấy theo nhóm (code, config, docs, ...).
fn node_tone(kind: &str) -> Color32 {
    let cat = node_category(kind);
    let i = NODE_CATEGORIES.iter().position(|&c| c == cat).unwrap_or(0);
    hex(LAYER_COLORS[i % LAYER_COLORS.len()])
}

/// Tiêu đề mục trong panel trái.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new(title).strong().size(13.0));
    ui.add_space(2.0);
}

// ---------------------------------------------------------------- bộ lọc (store.ts)

const NODE_CATEGORIES: [&str; 7] = ["code", "config", "docs", "infra", "data", "domain", "knowledge"];
const EDGE_CATEGORIES: [(&str, &[&str]); 9] = [
    ("structural", &["imports", "exports", "contains", "inherits", "implements"]),
    ("behavioral", &["calls", "subscribes", "publishes", "middleware"]),
    ("data-flow", &["reads_from", "writes_to", "transforms", "validates"]),
    ("dependencies", &["depends_on", "tested_by", "configures"]),
    ("semantic", &["related", "similar_to"]),
    ("infrastructure", &["deploys", "serves", "provisions", "triggers", "migrates", "documents", "routes", "defines_schema"]),
    ("domain", &["contains_flow", "flow_step", "cross_domain"]),
    ("knowledge", &["cites", "contradicts", "builds_on", "exemplifies", "categorized_under", "authored_by"]),
    ("design", &["instance_of", "variant_of", "uses_token"]),
];

fn node_category(kind: &str) -> &'static str {
    match kind {
        "config" => "config",
        "document" => "docs",
        "service" | "pipeline" | "resource" | "endpoint" => "infra",
        "table" | "schema" => "data",
        "domain" | "flow" | "step" => "domain",
        "article" | "entity" | "topic" | "claim" | "source" => "knowledge",
        _ => "code",
    }
}

fn edge_category(kind: &str) -> &'static str {
    for (cat, types) in EDGE_CATEGORIES {
        if types.contains(&kind) {
            return cat;
        }
    }
    "structural"
}

/// Persona (PersonaSelector.tsx): non-technical ẩn các node con cấp file (function, class).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Persona {
    NonTechnical,
    Junior,
    Experienced,
}

impl Persona {
    fn label(self) -> &'static str {
        match self {
            Persona::NonTechnical => "Không kỹ thuật",
            Persona::Junior => "Junior",
            Persona::Experienced => "Có kinh nghiệm",
        }
    }
    fn hides(self, kind: &str) -> bool {
        self == Persona::NonTechnical && matches!(kind, "function" | "class")
    }
}

// ---------------------------------------------------------------- trạng thái

#[derive(Clone, Copy, PartialEq, Debug)]
enum Level {
    Overview,
    Layer(usize),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum CardKind {
    Layer(usize),
    Node(usize),
    /// Cổng nối sang layer khác (PortalNode.tsx)
    Portal(usize),
}

struct Card {
    kind: CardKind,
    rect: Rect,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ViewSource {
    Structure,
    Domain,
    Knowledge,
}

struct App {
    project_root: PathBuf,
    graph_path: PathBuf,
    ua_dir: PathBuf,
    source: ViewSource,
    graph: Graph,
    load_error: Option<String>,

    by_id: HashMap<String, usize>,
    layer_of_node: Vec<Option<usize>>,
    degree: Vec<usize>,
    /// (layer a, layer b, số cạnh gốc) — aggregateLayerEdges
    layer_links: Vec<(usize, usize, usize)>,

    level: Level,
    cards: Vec<Card>,
    view_links: Vec<(usize, usize, f32)>,

    pan: Vec2,
    zoom: f32,
    fit_pending: u8,
    focus_request: Option<usize>,
    /// Vị trí thẻ do người dùng kéo: (loại, chỉ số) → góc trên-trái trong thế giới.
    pos_override: HashMap<(u8, usize), Pos2>,
    canvas_rect: Rect,

    selected_node: Option<usize>,
    highlight: HashSet<String>,
    path_nodes: HashSet<String>,
    show_all_edges: bool,
    minimap: bool,

    query: String,
    search_hits: Vec<usize>,

    cat_on: HashMap<&'static str, bool>,
    edge_cat_on: HashMap<&'static str, bool>,

    tour_active: bool,
    tour_step: usize,

    code_open: Option<usize>,
    code_text: Option<String>,
    show_help: bool,

    preset: usize,
    accent: usize,
    theme_open: bool,
    persona: Persona,

    diff: Option<DiffOverlay>,
    diff_on: bool,

    path_open: bool,
    path_from: Option<usize>,
    path_to: Option<usize>,
    path_result: Option<Vec<usize>>,
    path_filter: String,

    export_msg: Option<String>,
    show_left: bool,
    show_right: bool,
    /// Lịch sử điều hướng: quay lại / đi tiếp giữa tổng quan và các layer.
    back: Vec<Level>,
    forward: Vec<Level>,
}

impl App {
    fn new(path: &str) -> Self {
        let graph_path = PathBuf::new();
        let ua_dir = PathBuf::from(".");
        let project_root = PathBuf::from(".");

        let cat_on = NODE_CATEGORIES.iter().map(|&c| (c, true)).collect();
        let edge_cat_on = EDGE_CATEGORIES.iter().map(|&(c, _)| (c, true)).collect();

        let mut app = Self {
            project_root,
            graph_path,
            ua_dir,
            source: ViewSource::Structure,
            graph: Graph::default(),
            load_error: None,
            by_id: HashMap::new(),
            layer_of_node: Vec::new(),
            degree: Vec::new(),
            layer_links: Vec::new(),
            level: Level::Overview,
            cards: Vec::new(),
            view_links: Vec::new(),
            pan: Vec2::ZERO,
            zoom: 1.0,
            fit_pending: 60,
            focus_request: None,
            pos_override: HashMap::new(),
            canvas_rect: Rect::NOTHING,
            selected_node: None,
            highlight: HashSet::new(),
            path_nodes: HashSet::new(),
            show_all_edges: false,
            minimap: false,
            query: String::new(),
            search_hits: Vec::new(),
            cat_on,
            edge_cat_on,
            tour_active: false,
            tour_step: 0,
            code_open: None,
            code_text: None,
            show_help: false,
            preset: 0,
            accent: 0,
            theme_open: false,
            persona: Persona::Junior,
            diff: None,
            diff_on: false,
            path_open: false,
            path_from: None,
            path_to: None,
            path_result: None,
            path_filter: String::new(),
            export_msg: None,
            show_left: true,
            show_right: false,
            back: Vec::new(),
            forward: Vec::new(),
        };
        app.open_path(PathBuf::from(path));
        app
    }

    /// Thay toàn bộ dữ liệu đồ thị và dựng lại các chỉ mục.
    fn set_graph(&mut self, graph: Graph) {
        self.by_id = graph.nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
        let mut layer_of_node = vec![None; graph.nodes.len()];
        for (li, layer) in graph.layers.iter().enumerate() {
            for id in &layer.node_ids {
                if let Some(&i) = self.by_id.get(id) {
                    if layer_of_node[i].is_none() {
                        layer_of_node[i] = Some(li);
                    }
                }
            }
        }
        let mut degree = vec![0usize; graph.nodes.len()];
        let mut pair: HashMap<(usize, usize), usize> = HashMap::new();
        for e in &graph.edges {
            if let (Some(&s), Some(&t)) = (self.by_id.get(&e.source), self.by_id.get(&e.target)) {
                degree[s] += 1;
                degree[t] += 1;
                if let (Some(a), Some(b)) = (layer_of_node[s], layer_of_node[t]) {
                    if a != b {
                        let key = if a < b { (a, b) } else { (b, a) };
                        *pair.entry(key).or_insert(0) += 1;
                    }
                }
            }
        }
        let mut links: Vec<(usize, usize, usize)> = pair.into_iter().map(|((a, b), c)| (a, b, c)).collect();
        links.sort();
        self.graph = graph;
        self.layer_of_node = layer_of_node;
        self.degree = degree;
        self.layer_links = links;
        self.level = Level::Overview;
        self.back.clear();
        self.forward.clear();
        self.selected_node = None;
        self.highlight.clear();
        self.path_nodes.clear();
        self.search_hits.clear();
        self.rebuild_view();
    }

    fn switch_source(&mut self, src: ViewSource) {
        if self.source == src {
            return;
        }
        let file = match src {
            ViewSource::Structure => self.graph_path.clone(),
            ViewSource::Domain => self.ua_dir.join("domain-graph.json"),
            ViewSource::Knowledge => self.graph_path.clone(),
        };
        match std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str::<Graph>(&s).ok()) {
            Some(g) => {
                self.source = src;
                self.set_graph(g);
            }
            None => {
                self.export_msg = Some(format!("Không có dữ liệu cho chế độ này: {}", file.display()));
            }
        }
    }

    fn node_visible(&self, i: usize) -> bool {
        let n = &self.graph.nodes[i];
        *self.cat_on.get(node_category(&n.kind)).unwrap_or(&true) && !self.persona.hides(&n.kind)
    }

    fn edge_visible(&self, e: &Edge) -> bool {
        *self.edge_cat_on.get(edge_category(&e.kind)).unwrap_or(&true)
    }

    /// Các layer nối với layer `li`, kèm số cạnh (computePortals).
    fn portals_of(&self, li: usize) -> Vec<(usize, usize)> {
        let mut m: HashMap<usize, usize> = HashMap::new();
        for &(a, b, c) in &self.layer_links {
            if a == li {
                *m.entry(b).or_insert(0) += c;
            } else if b == li {
                *m.entry(a).or_insert(0) += c;
            }
        }
        let mut v: Vec<(usize, usize)> = m.into_iter().collect();
        v.sort();
        v
    }

    fn rebuild_view(&mut self) {
        self.cards.clear();
        self.view_links.clear();
        match self.level {
            Level::Overview => {
                let ids: Vec<String> = (0..self.graph.layers.len()).map(|i| format!("layer:{i}")).collect();
                let dims: Vec<(f32, f32)> = self.graph.layers.iter().map(|_| (300.0, 150.0)).collect();
                let community = vec![None; ids.len()];
                let links: Vec<(usize, usize)> = self.layer_links.iter().map(|&(a, b, _)| (a, b)).collect();
                let pos = layout::force_layout(&layout::Input { ids: &ids, dims: &dims, community: &community, links: &links });
                for (i, p) in pos.iter().enumerate() {
                    let at = self.pos_override.get(&(0, i)).copied().unwrap_or(Pos2::new(p.0, p.1));
                    self.cards.push(Card { kind: CardKind::Layer(i), rect: Rect::from_min_size(at, Vec2::new(300.0, 150.0)) });
                }
                for &(a, b, c) in &self.layer_links {
                    self.view_links.push((a, b, 1.0 + (c as f32).sqrt().min(6.0) * 0.6));
                }
            }
            Level::Layer(li) => {
                let members: Vec<usize> = (0..self.graph.nodes.len())
                    .filter(|&i| self.layer_of_node[i] == Some(li) && self.node_visible(i))
                    .collect();
                let local: HashMap<usize, usize> = members.iter().enumerate().map(|(k, &i)| (i, k)).collect();
                let ids: Vec<String> = members.iter().map(|&i| self.graph.nodes[i].id.clone()).collect();
                let dims: Vec<(f32, f32)> = members
                    .iter()
                    .map(|&i| {
                        let (w, h) = layout::node_dims(self.degree[i]);
                        (w * 0.7, h * 0.7)
                    })
                    .collect();
                let community = vec![None; members.len()];
                let mut links = Vec::new();
                for e in &self.graph.edges {
                    if !self.edge_visible(e) {
                        continue;
                    }
                    if let (Some(&s), Some(&t)) = (self.by_id.get(&e.source), self.by_id.get(&e.target)) {
                        if let (Some(&ls), Some(&lt)) = (local.get(&s), local.get(&t)) {
                            links.push((ls, lt));
                            self.view_links.push((ls, lt, 1.0));
                        }
                    }
                }
                let pos = layout::force_layout(&layout::Input { ids: &ids, dims: &dims, community: &community, links: &links });
                for (k, &i) in members.iter().enumerate() {
                    let (w, h) = dims[k];
                    let at = self.pos_override.get(&(1, i)).copied().unwrap_or(Pos2::new(pos[k].0, pos[k].1));
                    self.cards.push(Card { kind: CardKind::Node(i), rect: Rect::from_min_size(at, Vec2::new(w, h)) });
                }
                // Cổng nối: đặt ở bên phải khối node, xếp dọc.
                if let Some(bounds) = self.cards.iter().map(|c| c.rect).reduce(|a, b| a.union(b)) {
                    for (k, (other, _count)) in self.portals_of(li).into_iter().enumerate() {
                        let pos = Pos2::new(bounds.max.x + 120.0, bounds.min.y + k as f32 * 96.0);
                        let at = self.pos_override.get(&(2, other)).copied().unwrap_or(pos);
                        self.cards.push(Card { kind: CardKind::Portal(other), rect: Rect::from_min_size(at, Vec2::new(230.0, 70.0)) });
                    }
                }
            }
        }
        self.fit_pending = 60;
    }

    /// Chuyển cấp xem và ghi lịch sử (để quay lại / đi tiếp).
    fn navigate_level(&mut self, lv: Level) {
        if lv != self.level {
            self.back.push(self.level);
            self.forward.clear();
            self.level = lv;
        }
        self.rebuild_view();
    }

    fn enter_layer(&mut self, li: usize) {
        self.navigate_level(Level::Layer(li));
        self.selected_node = None;
    }

    fn go_overview(&mut self) {
        self.navigate_level(Level::Overview);
    }

    fn go_back(&mut self) {
        if let Some(lv) = self.back.pop() {
            self.forward.push(self.level);
            self.level = lv;
            self.selected_node = None;
            self.rebuild_view();
        }
    }

    fn go_forward(&mut self) {
        if let Some(lv) = self.forward.pop() {
            self.back.push(self.level);
            self.level = lv;
            self.selected_node = None;
            self.rebuild_view();
        }
    }

    fn navigate_to_node(&mut self, ni: usize) {
        if let Some(li) = self.layer_of_node[ni] {
            if self.level != Level::Layer(li) {
                self.enter_layer(li);
            }
        }
        self.selected_node = Some(ni);
        self.focus_request = Some(ni);
    }

    fn world_bounds(&self) -> Option<Rect> {
        self.cards.iter().map(|c| c.rect).reduce(|a, b| a.union(b))
    }

    fn name_of(&self, id: &str) -> String {
        self.by_id.get(id).map(|&i| self.graph.nodes[i].name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// Zoom quanh điểm `anchor` (tọa độ tương đối canvas), giữ điểm đó đứng yên trên màn hình.
    fn zoom_at(&mut self, anchor: Vec2, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(0.08, 2.5);
        let before = (anchor - self.pan) / self.zoom;
        self.zoom = new_zoom;
        self.pan = anchor - before * new_zoom;
        self.fit_pending = 0;
    }

    fn card_screen_rect(&self, origin: Pos2, r: Rect) -> Rect {
        Rect::from_two_pos(origin + self.pan + r.min.to_vec2() * self.zoom, origin + self.pan + r.max.to_vec2() * self.zoom)
    }

    fn run_search(&mut self) {
        self.search_hits.clear();
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            return;
        }
        for (i, n) in self.graph.nodes.iter().enumerate() {
            if !self.node_visible(i) {
                continue;
            }
            let hay = format!("{} {} {}", n.name, n.summary, n.tags.join(" ")).to_lowercase();
            if hay.contains(&q) {
                self.search_hits.push(i);
            }
            if self.search_hits.len() >= 60 {
                break;
            }
        }
    }

    fn tour_highlight(&mut self) {
        if let Some(step) = self.graph.tour.get(self.tour_step).cloned() {
            self.highlight = step.node_ids.iter().cloned().collect();
            if let Some(&first) = step.node_ids.iter().find_map(|id| self.by_id.get(id)) {
                self.navigate_to_node(first);
            }
        }
    }

    /// BFS hai chiều, giống PathFinderModal.tsx.
    fn find_path(&self, from: usize, to: usize) -> Vec<usize> {
        if from == to {
            return vec![from];
        }
        let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
        for e in &self.graph.edges {
            if let (Some(&s), Some(&t)) = (self.by_id.get(&e.source), self.by_id.get(&e.target)) {
                adj.entry(s).or_default().push(t);
                adj.entry(t).or_default().push(s);
            }
        }
        let mut prev: HashMap<usize, usize> = HashMap::new();
        let mut seen = HashSet::from([from]);
        let mut q = VecDeque::from([from]);
        while let Some(cur) = q.pop_front() {
            if cur == to {
                let mut path = vec![to];
                let mut c = to;
                while let Some(&p) = prev.get(&c) {
                    path.push(p);
                    c = p;
                }
                path.reverse();
                return path;
            }
            for &nb in adj.get(&cur).into_iter().flatten() {
                if seen.insert(nb) {
                    prev.insert(nb, cur);
                    q.push_back(nb);
                }
            }
        }
        Vec::new()
    }

    fn load_code(&mut self, ni: usize) {
        self.code_open = Some(ni);
        let text = match self.graph.nodes[ni].file_path.clone() {
            None => "Node này không có đường dẫn file.".to_string(),
            Some(rel) => {
                let p = self.project_root.join(&rel);
                match std::fs::read(&p) {
                    Ok(bytes) if bytes.len() <= 1_000_000 => String::from_utf8_lossy(&bytes).into_owned(),
                    Ok(_) => "File quá lớn để hiển thị (trên 1 MB).".to_string(),
                    Err(e) => format!("Không đọc được {}: {e}", p.display()),
                }
            }
        };
        self.code_text = Some(text);
    }

    // ------------------------------------------------------------ xuất (ExportMenu.tsx)

    fn export(&mut self, fmt: &str) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (name, body) = match fmt {
            "json" => (format!("ua-export-{stamp}.json"), serde_json::to_string_pretty(&serde_json::json!({
                "nodes": self.graph.nodes.iter().map(|n| serde_json::json!({"id": n.id, "type": n.kind, "name": n.name, "summary": n.summary, "filePath": n.file_path})).collect::<Vec<_>>(),
                "edges": self.graph.edges.iter().map(|e| serde_json::json!({"source": e.source, "target": e.target, "type": e.kind})).collect::<Vec<_>>(),
                "layers": self.graph.layers.iter().map(|l| serde_json::json!({"id": l.id, "name": l.name, "nodeIds": l.node_ids})).collect::<Vec<_>>(),
            })).unwrap_or_default()),
            "md" => {
                let mut s = String::new();
                let _ = writeln!(s, "# {}\n", self.source_label());
                for layer in &self.graph.layers {
                    let _ = writeln!(s, "## {}\n\n{}\n", layer.name, layer.description);
                    for id in &layer.node_ids {
                        if let Some(&i) = self.by_id.get(id) {
                            let n = &self.graph.nodes[i];
                            let _ = writeln!(s, "- **{}** ({}): {}", n.name, n.kind, n.summary);
                        }
                    }
                    s.push('\n');
                }
                (format!("ua-export-{stamp}.md"), s)
            }
            _ => {
                // SVG: các thẻ đang hiển thị, theo đúng tọa độ thế giới.
                let b = self.world_bounds().unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0)));
                let (w, h) = (b.width() + 40.0, b.height() + 40.0);
                let mut s = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"##);
                s.push_str(r##"<rect width="100%" height="100%" fill="#ffffff"/>"##);
                let off = |p: Pos2| (p.x - b.min.x + 20.0, p.y - b.min.y + 20.0);
                for c in &self.cards {
                    let (x, y) = off(c.rect.min);
                    let label = match c.kind {
                        CardKind::Layer(li) => self.graph.layers[li].name.clone(),
                        CardKind::Node(ni) => self.graph.nodes[ni].name.clone(),
                        CardKind::Portal(li) => self.graph.layers[li].name.clone(),
                    };
                    let _ = write!(s, r##"<rect x="{x}" y="{y}" width="{}" height="{}" rx="8" fill="#ffffff" stroke="#282828"/><text x="{}" y="{}" font-family="Inter, sans-serif" font-size="13" fill="#000000">{}</text>"##,
                        c.rect.width(), c.rect.height(), x + 10.0, y + 20.0, xml_escape(&label));
                }
                s.push_str("</svg>");
                (format!("ua-export-{stamp}.svg"), s)
            }
        };
        let path = self.project_root.join(&name);
        self.export_msg = Some(match std::fs::write(&path, body) {
            Ok(_) => format!("Đã xuất: {}", path.display()),
            Err(e) => format!("Xuất thất bại: {e}"),
        });
    }

    /// Mở một file knowledge-graph.json hoặc thư mục dự án (có .ua/ hoặc .understand-anything/).
    fn open_path(&mut self, p: PathBuf) {
        let file = if p.is_dir() {
            [".ua", ".understand-anything"]
                .iter()
                .map(|d| p.join(d).join("knowledge-graph.json"))
                .find(|f| f.is_file())
                .unwrap_or_else(|| p.join(".ua").join("knowledge-graph.json"))
        } else {
            p
        };
        let ua_dir = file.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        let project_root = if ua_dir.file_name().map(|n| n == ".ua" || n == ".understand-anything").unwrap_or(false) {
            ua_dir.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
        } else {
            ua_dir.clone()
        };
        let parsed = std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Graph>(&s).map_err(|e| e.to_string()));
        match parsed {
            Ok(g) => {
                self.diff = std::fs::read_to_string(ua_dir.join("diff-overlay.json"))
                    .ok()
                    .and_then(|s| serde_json::from_str::<DiffOverlay>(&s).ok());
                self.diff_on = false;
                self.project_root = project_root;
                self.graph_path = file;
                self.ua_dir = ua_dir;
                self.source = ViewSource::Structure;
                self.load_error = None;
                self.pos_override.clear();
                self.set_graph(g);
                self.fit_pending = 60;
            }
            Err(e) => {
                self.load_error = Some(format!("{}: {e}", file.display()));
                self.set_graph(Graph::default());
            }
        }
    }

    fn import_file(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("knowledge-graph", &["json"]).pick_file() {
            self.open_path(p);
        }
    }

    fn import_folder(&mut self) {
        if let Some(p) = rfd::FileDialog::new().pick_folder() {
            self.open_path(p);
        }
    }

    fn source_label(&self) -> String {
        self.project_root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "knowledge graph".into())
    }
}

/// Bóng đổ mềm: ba lớp alpha giảm dần, lệch xuống, tạo độ sâu không cần blur.
fn card_shadow(painter: &egui::Painter, rect: Rect, radius: f32) {
    for (grow, dy, alpha) in [(6.0, 6.0, 6u8), (3.0, 3.0, 10u8), (1.0, 1.0, 16u8)] {
        let r = rect.expand(grow).translate(Vec2::new(0.0, dy));
        painter.rect_filled(r, radius + grow, Color32::from_black_alpha(alpha));
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

// ---------------------------------------------------------------- font (SF Pro → Inter Display)

static FONT_REGULAR: &[u8] = include_bytes!("../assets/InterDisplay-Regular.ttf");
static FONT_MEDIUM: &[u8] = include_bytes!("../assets/InterDisplay-Medium.ttf");
static FONT_SEMIBOLD: &[u8] = include_bytes!("../assets/InterDisplay-SemiBold.ttf");

fn setup_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("inter-regular".into(), Arc::new(FontData::from_static(FONT_REGULAR)));
    fonts.font_data.insert("inter-medium".into(), Arc::new(FontData::from_static(FONT_MEDIUM)));
    fonts.font_data.insert("inter-semibold".into(), Arc::new(FontData::from_static(FONT_SEMIBOLD)));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter-regular".into());
    fonts.families.insert(FontFamily::Name("medium".into()), vec!["inter-medium".into(), "inter-regular".into()]);
    fonts.families.insert(FontFamily::Name("semibold".into()), vec!["inter-semibold".into(), "inter-regular".into()]);
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------- giao diện

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let pal = palette(&PRESETS[self.preset], self.accent);
        let mut visuals = if PRESETS[self.preset].is_dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        visuals.panel_fill = pal.surface;
        visuals.window_fill = pal.surface;
        visuals.override_text_color = Some(pal.ink);
        // Nút và ô nhập: một kiểu phẳng, bo góc nhỏ, viền mảnh (thay cho nút xám mặc định)
        let radius = egui::CornerRadius::same(6);
        for w in [&mut visuals.widgets.inactive, &mut visuals.widgets.hovered, &mut visuals.widgets.active, &mut visuals.widgets.open] {
            w.corner_radius = radius;
        }
        visuals.widgets.inactive.weak_bg_fill = pal.surface;
        visuals.widgets.inactive.bg_fill = pal.surface;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, pal.border);
        let hover_fill = mix(pal.surface, pal.border, 0.6);
        visuals.widgets.hovered.weak_bg_fill = hover_fill;
        visuals.widgets.hovered.bg_fill = hover_fill;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, mix(pal.surface, pal.ink, 0.25));
        let press_fill = mix(pal.surface, pal.border, 0.9);
        visuals.widgets.active.weak_bg_fill = press_fill;
        visuals.widgets.active.bg_fill = press_fill;
        visuals.selection.bg_fill = mix(pal.surface, pal.accent, 0.22);
        ctx.set_visuals(visuals);

        // Kéo thả file knowledge-graph.json hoặc thư mục dự án vào cửa sổ để mở
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).find(|p| !p.as_os_str().is_empty())) {
            self.open_path(p);
        }

        // Hẹp: ẩn panel bên để dành chỗ cho canvas (layout mobile)
        let narrow = ctx.content_rect().width() < 760.0;

        // Phím tắt
        let (esc, slash, left, right, help, minimap_key, fit_key) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Slash),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::H),
                i.key_pressed(egui::Key::M),
                i.key_pressed(egui::Key::F),
            )
        });
        if esc {
            if self.code_open.is_some() {
                self.code_open = None;
            } else if self.show_help {
                self.show_help = false;
            } else if self.path_open {
                self.path_open = false;
            } else if self.tour_active {
                self.tour_active = false;
            } else if self.level != Level::Overview {
                self.go_overview();
            }
        }
        if help {
            self.show_help = !self.show_help;
        }
        if minimap_key {
            self.minimap = !self.minimap;
        }
        if slash {
            ctx.memory_mut(|m| m.request_focus(Id::new("search-box")));
        }
        if self.tour_active && (left || right) {
            let n = self.graph.tour.len();
            if n > 0 {
                self.tour_step = if right { (self.tour_step + 1).min(n - 1) } else { self.tour_step.saturating_sub(1) };
                self.tour_highlight();
            }
        }

        // Thanh trên tối giản: tên dự án, mở dự án, chi tiết, và menu "Tùy chọn" cho phần còn lại.
        egui::Panel::top("top")
            .frame(egui::Frame::new().fill(pal.surface).stroke(Stroke::new(1.0, pal.border)).inner_margin(egui::Margin::symmetric(12, 6)))
            .show_inside(root, |ui| {
            ui.horizontal(|ui| {
                if narrow && ui.button(if self.show_left { "Ẩn danh sách" } else { "Danh sách" }).clicked() {
                    self.show_left = !self.show_left;
                }
                ui.label(egui::RichText::new(self.source_label()).strong().size(15.0));
                ui.menu_button("Mở", |ui| {
                    if ui.button("Mở file knowledge-graph.json…").clicked() {
                        self.import_file();
                        ui.close();
                    }
                    if ui.button("Mở thư mục dự án…").clicked() {
                        self.import_folder();
                        ui.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("Tùy chọn", |ui| {
                        if ui.button("Tìm đường đi…").clicked() {
                            self.path_open = !self.path_open;
                            ui.close();
                        }
                        if ui.button("Giao diện…").clicked() {
                            self.theme_open = !self.theme_open;
                            ui.close();
                        }
                        ui.menu_button("Xuất", |ui| {
                            if ui.button("SVG").clicked() {
                                self.export("svg");
                                ui.close();
                            }
                            if ui.button("Markdown").clicked() {
                                self.export("md");
                                ui.close();
                            }
                            if ui.button("JSON").clicked() {
                                self.export("json");
                                ui.close();
                            }
                        });
                        ui.separator();
                        ui.checkbox(&mut self.minimap, "Bản đồ nhỏ (M)");
                        ui.checkbox(&mut self.show_all_edges, "Hiện mọi cạnh");
                        if self.diff.is_some() {
                            ui.checkbox(&mut self.diff_on, "Hiện thay đổi (diff)");
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Chế độ xem").color(pal.muted).small());
                        for p in [Persona::NonTechnical, Persona::Junior, Persona::Experienced] {
                            ui.radio_value(&mut self.persona, p, p.label());
                        }
                    });
                    if ui.selectable_label(self.show_right, "Chi tiết").clicked() {
                        self.show_right = !self.show_right;
                    }
                });
            });
            if let Some(msg) = &self.export_msg {
                ui.label(egui::RichText::new(msg).color(pal.muted).small());
            }
        });

        // Theme: chọn preset và accent (ThemePicker.tsx)
        if self.theme_open {
            egui::Window::new("Theme").collapsible(false).resizable(false).show(&ctx, |ui| {
                for (i, p) in PRESETS.iter().enumerate() {
                    if ui.selectable_label(self.preset == i, p.name).clicked() {
                        self.preset = i;
                        self.accent = 0;
                    }
                }
                ui.separator();
                let preset = &PRESETS[self.preset];
                ui.horizontal_wrapped(|ui| {
                    for (k, (name, col)) in preset.accents.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                        ui.painter().rect_filled(rect, 4.0, hex(col));
                        if self.accent == k {
                            ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0, pal.ink), egui::StrokeKind::Outside);
                        }
                        if resp.on_hover_text(*name).clicked() {
                            self.accent = k;
                        }
                    }
                });
                if ui.button("Đóng").clicked() {
                    self.theme_open = false;
                }
            });
        }

        // Panel trái: nguồn, tìm kiếm, layer có màu, bộ lọc, tour
        if self.show_left || !narrow {
            egui::Panel::left("left")
                .resizable(true)
                .default_size(272.0)
                .frame(egui::Frame::new().fill(pal.surface).stroke(Stroke::new(1.0, pal.border)).inner_margin(egui::Margin::symmetric(14, 14)))
                .show_inside(root, |ui| {
                ui.label(
                    egui::RichText::new(format!("{} nodes · {} edges · {} layers", self.graph.nodes.len(), self.graph.edges.len(), self.graph.layers.len()))
                        .color(pal.muted)
                        .size(12.0),
                );

                ui.add_space(10.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .id(Id::new("search-box"))
                        .hint_text("Tìm node…  ( / )")
                        .desired_width(f32::INFINITY),
                );
                if resp.changed() {
                    self.run_search();
                }
                if !self.search_hits.is_empty() {
                    ui.label(egui::RichText::new(format!("{} kết quả", self.search_hits.len())).color(pal.muted).size(12.0));
                    let hits = self.search_hits.clone();
                    egui::ScrollArea::vertical().max_height(160.0).id_salt("hits").show(ui, |ui| {
                        for ni in hits {
                            let label = format!("{}  · {}", self.graph.nodes[ni].name, self.graph.nodes[ni].kind);
                            if ui.selectable_label(false, label).clicked() {
                                self.navigate_to_node(ni);
                            }
                        }
                    });
                }

                section(ui, "Layer");
                let mut open = None;
                for (i, layer) in self.graph.layers.iter().enumerate() {
                    let selected = self.level == Level::Layer(i);
                    let tone = hex(LAYER_COLORS[i % LAYER_COLORS.len()]);
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                    let bg = if selected {
                        mix(pal.surface, tone, 0.14)
                    } else if resp.hovered() {
                        mix(pal.surface, tone, 0.06)
                    } else {
                        pal.surface
                    };
                    let p = ui.painter();
                    p.rect_filled(rect, 6.0, bg);
                    p.circle_filled(Pos2::new(rect.min.x + 12.0, rect.center().y), 4.5, tone);
                    p.text(
                        Pos2::new(rect.min.x + 26.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        truncate(&layer.name, 24),
                        FontId::new(13.0, egui::FontFamily::Name("medium".into())),
                        pal.ink,
                    );
                    p.text(
                        Pos2::new(rect.max.x - 10.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        layer.node_ids.len().to_string(),
                        FontId::proportional(12.0),
                        pal.muted,
                    );
                    if resp.clicked() {
                        open = Some(i);
                    }
                }
                if let Some(i) = open {
                    self.enter_layer(i);
                }

                ui.add_space(10.0);
                egui::CollapsingHeader::new("Bộ lọc").default_open(false).show(ui, |ui| {
                    let mut changed = false;
                    for c in NODE_CATEGORIES {
                        let mut on = *self.cat_on.get(c).unwrap_or(&true);
                        if ui.checkbox(&mut on, format!("Loại: {c}")).changed() {
                            self.cat_on.insert(c, on);
                            changed = true;
                        }
                    }
                    for (c, _) in EDGE_CATEGORIES {
                        let mut on = *self.edge_cat_on.get(c).unwrap_or(&true);
                        if ui.checkbox(&mut on, format!("Cạnh: {c}")).changed() {
                            self.edge_cat_on.insert(c, on);
                            changed = true;
                        }
                    }
                    if changed && matches!(self.level, Level::Layer(_)) {
                        self.rebuild_view();
                    }
                });
                egui::CollapsingHeader::new(format!("Tour · {} bước", self.graph.tour.len())).default_open(false).show(ui, |ui| {
                    if self.graph.tour.is_empty() {
                        ui.label(egui::RichText::new("Dự án này chưa có tour.").color(pal.muted));
                    } else if self.tour_active {
                        let n = self.graph.tour.len();
                        if let Some(step) = self.graph.tour.get(self.tour_step).cloned() {
                            ui.horizontal(|ui| {
                                if ui.button("◀").clicked() && self.tour_step > 0 {
                                    self.tour_step -= 1;
                                    self.tour_highlight();
                                }
                                ui.label(format!("{} / {}", self.tour_step + 1, n));
                                if ui.button("▶").clicked() && self.tour_step + 1 < n {
                                    self.tour_step += 1;
                                    self.tour_highlight();
                                }
                                if ui.small_button("Thoát").clicked() {
                                    self.tour_active = false;
                                    self.highlight.clear();
                                }
                            });
                            ui.label(egui::RichText::new(format!("{}. {}", step.order, step.title)).strong());
                            ui.label(egui::RichText::new(&step.description).color(pal.muted).size(12.0));
                        }
                    } else {
                        for (i, step) in self.graph.tour.clone().iter().enumerate() {
                            if ui.selectable_label(false, format!("{}. {}", step.order, step.title)).clicked() {
                                self.tour_active = true;
                                self.tour_step = i;
                                self.tour_highlight();
                            }
                        }
                    }
                });

            });
        }

        // Panel phải: chi tiết node (NodeInfo.tsx), ẩn mặc định, mở bằng nút "Chi tiết"
        if self.show_right {
            egui::Panel::right("right").resizable(true).default_size(300.0).show_inside(root, |ui| {
                ui.strong("Chi tiết");
                ui.add_space(4.0);
                egui::ScrollArea::vertical().id_salt("right-scroll").auto_shrink([false, false]).show(ui, |ui| match self.selected_node {
                    None => {
                        ui.label(egui::RichText::new("Bấm một thẻ để xem chi tiết.").color(pal.muted));
                    }
                    Some(i) => {
                        let node = self.graph.nodes[i].clone();
                        ui.label(egui::RichText::new(&node.name).strong().size(17.0));
                        ui.label(egui::RichText::new(format!("{} · {}", node.kind, node.complexity)).color(pal.muted));
                        if let Some(p) = &node.file_path {
                            ui.label(egui::RichText::new(p).monospace().small());
                        }
                        ui.add_space(6.0);
                        ui.label(&node.summary);
                        if !node.tags.is_empty() {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(node.tags.join(" · ")).color(pal.muted).small());
                        }
                        if node.file_path.is_some() && ui.button("Xem mã nguồn").clicked() {
                            self.load_code(i);
                        }
                        ui.separator();
                        ui.strong("Kết nối");
                        egui::ScrollArea::vertical().id_salt("conn").show(ui, |ui| {
                            let id = node.id.clone();
                            let mut clicked = None;
                            for e in &self.graph.edges {
                                if !self.edge_visible(e) {
                                    continue;
                                }
                                let (text, other) = if e.source == id {
                                    (format!("→ {} ({})", self.name_of(&e.target), e.kind), e.target.clone())
                                } else if e.target == id {
                                    (format!("← {} ({})", self.name_of(&e.source), e.kind), e.source.clone())
                                } else {
                                    continue;
                                };
                                if ui.selectable_label(false, text).clicked() {
                                    clicked = self.by_id.get(&other).copied();
                                }
                            }
                            if let Some(ni) = clicked {
                                self.navigate_to_node(ni);
                            }
                        });
                    }
                });
            });
        }

        // Cửa sổ đường đi (PathFinderModal.tsx)
        if self.path_open {
            let mut open = true;
            let names: Vec<(usize, String)> = self.graph.nodes.iter().enumerate().map(|(i, n)| (i, format!("{} · {}", n.name, n.kind))).collect();
            egui::Window::new("Tìm đường đi ngắn nhất").open(&mut open).default_width(420.0).show(&ctx, |ui| {
                ui.label("Từ:");
                pick_node(ui, "from", &names, &mut self.path_from, &mut self.path_filter);
                ui.label("Đến:");
                pick_node(ui, "to", &names, &mut self.path_to, &mut self.path_filter);
                ui.horizontal(|ui| {
                    if ui.button("Tìm đường").clicked() {
                        self.path_result = match (self.path_from, self.path_to) {
                            (Some(a), Some(b)) => Some(self.find_path(a, b)),
                            _ => None,
                        };
                        self.path_nodes = self.path_result.iter().flatten().map(|&i| self.graph.nodes[i].id.clone()).collect();
                    }
                    if ui.button("Xóa").clicked() {
                        self.path_result = None;
                        self.path_nodes.clear();
                    }
                });
                match &self.path_result {
                    None => {}
                    Some(p) if p.is_empty() => {
                        ui.colored_label(Color32::from_rgb(180, 0, 0), "Không có đường đi giữa hai node này.");
                    }
                    Some(p) => {
                        ui.label(format!("{} bước:", p.len().saturating_sub(1)));
                        for &i in p {
                            ui.label(format!("· {}", self.graph.nodes[i].name));
                        }
                    }
                }
            });
            self.path_open = open;
        }

        // Trợ giúp phím tắt
        if self.show_help {
            let mut close = false;
            egui::Window::new("Phím tắt").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(&ctx, |ui| {
                ui.label("F — căn cho vừa khung nhìn");
                ui.label("Esc — đóng cửa sổ, thoát tour, hoặc về tổng quan");
                ui.label("/ — ô tìm kiếm");
                ui.label("← → — bước tour");
                ui.label("M — bật/tắt minimap");
                ui.label("H — bật/tắt phím tắt");
                ui.label("Kéo nền để di chuyển, cuộn để phóng to/thu nhỏ");
                if ui.button("Đóng").clicked() {
                    close = true;
                }
            });
            if close {
                self.show_help = false;
            }
        }

        // Cửa sổ mã nguồn (CodeViewer.tsx)
        if let Some(ni) = self.code_open {
            let title = self.graph.nodes[ni].file_path.clone().unwrap_or_default();
            let text = self.code_text.clone().unwrap_or_default();
            let mut open = true;
            egui::Window::new(format!("Mã nguồn — {title}")).open(&mut open).default_size([760.0, 520.0]).show(&ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    for (k, line) in text.lines().enumerate() {
                        ui.label(egui::RichText::new(format!("{:>5}  {line}", k + 1)).monospace().size(12.0));
                    }
                });
            });
            if !open {
                self.code_open = None;
            }
        }

        // Canvas
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.bg)).show(root, |ui| {
            // Chưa có dữ liệu: màn hình mở dự án
            if self.graph.nodes.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 3.0);
                    ui.label(egui::RichText::new("Chưa có dự án nào được mở").strong().size(20.0));
                    ui.label(egui::RichText::new("Mở file knowledge-graph.json hoặc thư mục dự án có thư mục .ua/. Cũng có thể kéo thả file vào cửa sổ.").color(pal.muted));
                    if let Some(err) = &self.load_error {
                        ui.add_space(8.0);
                        ui.colored_label(Color32::from_rgb(180, 0, 0), truncate(err, 140));
                    }
                    ui.add_space(14.0);
                    if ui.button("Mở file knowledge-graph.json…").clicked() {
                        self.import_file();
                    }
                    if ui.button("Mở thư mục dự án…").clicked() {
                        self.import_folder();
                    }
                });
                return;
            }
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let origin = response.rect.min;
            self.canvas_rect = response.rect;

            if let Some(ni) = self.focus_request.take() {
                if let Some(card) = self.cards.iter().find(|c| c.kind == CardKind::Node(ni)) {
                    let c = card.rect.center();
                    self.zoom = self.zoom.max(0.9);
                    self.pan = response.rect.size() / 2.0 - c.to_vec2() * self.zoom;
                    self.fit_pending = 0;
                }
            } else if self.fit_pending > 0 || fit_key {
                self.fit_pending = self.fit_pending.saturating_sub(1);
                if let Some(b) = self.world_bounds() {
                    let avail = response.rect.size() - Vec2::splat(40.0);
                    let z = (avail.x / b.width()).min(avail.y / b.height()).clamp(0.08, 1.2);
                    self.zoom = z;
                    self.pan = response.rect.size() / 2.0 - b.center().to_vec2() * z;
                }
            }

            if response.dragged() {
                self.pan += response.drag_delta();
                self.fit_pending = 0;
            }
            // Cuộn chuột / pinch để zoom quanh con trỏ. Dùng rect_contains_pointer vì thẻ
            // (interact đặt sau canvas) có thể chiếm hover của canvas.
            if ui.rect_contains_pointer(response.rect) {
                let (scroll, pinch, pointer) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta(), i.pointer.hover_pos()));
                if scroll != 0.0 || pinch != 1.0 {
                    let factor = pinch * (scroll * 0.0015).exp();
                    let pointer = pointer.unwrap_or(response.rect.center());
                    self.zoom_at(pointer - origin, factor);
                }
            }

            // Nút zoom góc dưới-trái canvas
            let mut zoom_step = None;
            let mut fit_click = false;
            let mut back_click = false;
            let mut forward_click = false;
            egui::Area::new(Id::new("zoom-controls"))
                .fixed_pos(response.rect.left_top() + Vec2::new(12.0, 34.0))
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(pal.surface)
                        .stroke(Stroke::new(1.0, pal.border))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.back.is_empty(), egui::Button::new("← Quay lại")).clicked() {
                            back_click = true;
                        }
                        if ui.add_enabled(!self.forward.is_empty(), egui::Button::new("Đi tiếp →")).clicked() {
                            forward_click = true;
                        }
                        ui.separator();
                        if ui.button("+").on_hover_text("Phóng to").clicked() {
                            zoom_step = Some(1.25);
                        }
                        if ui.button("−").on_hover_text("Thu nhỏ").clicked() {
                            zoom_step = Some(0.8);
                        }
                        if ui.button("Vừa khung").on_hover_text("Căn cho vừa khung nhìn (F)").clicked() {
                            fit_click = true;
                        }
                    });
                        });
                });
            if let Some(f) = zoom_step {
                self.zoom_at(response.rect.size() / 2.0, f);
            }
            if fit_click {
                self.fit_pending = 60;
            }
            if back_click {
                self.go_back();
            }
            if forward_click {
                self.go_forward();
            }

            let painter = painter.with_clip_rect(response.rect);
            // Nền lưới chấm, bám theo pan để có cảm giác không gian
            let step = 28.0;
            let mut gx = origin.x + self.pan.x.rem_euclid(step);
            while gx < response.rect.max.x {
                let mut gy = origin.y + self.pan.y.rem_euclid(step);
                while gy < response.rect.max.y {
                    painter.circle_filled(Pos2::new(gx, gy), 1.0, pal.border);
                    gy += step;
                }
                gx += step;
            }
            let screen: Vec<Rect> = self.cards.iter().map(|c| self.card_screen_rect(origin, c.rect)).collect();
            let centers: Vec<Pos2> = screen.iter().map(|r| r.center()).collect();
            let sel_id = self.selected_node.map(|i| self.graph.nodes[i].id.clone());

            // Cạnh
            for &(a, b, width) in &self.view_links {
                let (pa, pb) = (centers[a], centers[b]);
                match self.level {
                    Level::Overview => {
                        painter.line_segment([pa, pb], Stroke::new(width, pal.edge));
                    }
                    Level::Layer(_) => {
                        let id_of = |k: CardKind| match k {
                            CardKind::Node(i) => Some(self.graph.nodes[i].id.clone()),
                            _ => None,
                        };
                        let (ida, idb) = (id_of(self.cards[a].kind), id_of(self.cards[b].kind));
                        let on_path = ida.as_ref().is_some_and(|s| self.path_nodes.contains(s)) && idb.as_ref().is_some_and(|s| self.path_nodes.contains(s));
                        let incident = sel_id.as_ref().is_some_and(|s| Some(s) == ida.as_ref() || Some(s) == idb.as_ref())
                            || ida.as_ref().is_some_and(|s| self.highlight.contains(s))
                            || idb.as_ref().is_some_and(|s| self.highlight.contains(s));
                        if on_path {
                            painter.line_segment([pa, pb], Stroke::new(3.0, pal.accent));
                        } else if incident || self.show_all_edges {
                            let color = if incident { pal.ink } else { pal.edge };
                            painter.line_segment([pa, pb], Stroke::new(if incident { 2.0 } else { 1.0 }, color));
                        }
                    }
                }
            }

            // Thẻ
            let mut drags: Vec<(usize, Vec2)> = Vec::new();
            let mut next_level = None;
            let mut next_node = None;
            let mut code_req = None;
            for (ci, card) in self.cards.iter().enumerate() {
                let rect = screen[ci];
                let resp = ui.interact(rect, Id::new(("card", ci)), Sense::click_and_drag());
                if resp.dragged() {
                    drags.push((ci, resp.drag_delta()));
                }
                match card.kind {
                    CardKind::Layer(li) => {
                        let layer = &self.graph.layers[li];
                        let s = self.zoom.max(0.4);
                        let tone = hex(LAYER_COLORS[li % LAYER_COLORS.len()]);
                        card_shadow(&painter, rect, 12.0 * s);
                        painter.rect(rect, 12.0 * s, mix(pal.surface, tone, 0.04), Stroke::new(1.0, mix(pal.surface, tone, 0.3)), egui::StrokeKind::Inside);
                        let inner = painter.with_clip_rect(rect.shrink(4.0));
                        inner.text(rect.min + Vec2::new(16.0 * s, 16.0 * s), Align2::LEFT_TOP, truncate(&layer.name, 24), FontId::new(19.0 * s, egui::FontFamily::Name("semibold".into())), pal.ink);
                        inner.text(rect.min + Vec2::new(16.0 * s, 46.0 * s), Align2::LEFT_TOP, truncate(&layer.description, 48), FontId::proportional(12.0 * s), pal.muted);
                        inner.text(rect.max - Vec2::new(14.0 * s, 14.0 * s), Align2::RIGHT_BOTTOM, format!("{} files", layer.node_ids.len()), FontId::new(11.5 * s, egui::FontFamily::Name("semibold".into())), tone);
                        if resp.clicked() {
                            next_level = Some(li);
                        }
                    }
                    CardKind::Portal(other) => {
                        let s = self.zoom.max(0.4);
                        let tone = hex(LAYER_COLORS[other % LAYER_COLORS.len()]);
                        card_shadow(&painter, rect, 12.0 * s);
                        painter.rect(rect, 12.0 * s, mix(pal.surface, tone, 0.12), Stroke::new(1.0, tone), egui::StrokeKind::Inside);
                        let inner = painter.with_clip_rect(rect.shrink(4.0));
                        inner.text(rect.min + Vec2::new(14.0 * s, 12.0 * s), Align2::LEFT_TOP, "CỔNG NỐI →", FontId::new(10.0 * s, egui::FontFamily::Name("semibold".into())), tone);
                        inner.text(rect.min + Vec2::new(14.0 * s, 30.0 * s), Align2::LEFT_TOP, truncate(&self.graph.layers[other].name, 24), FontId::new(14.0 * s, egui::FontFamily::Name("medium".into())), pal.ink);
                        if resp.clicked() {
                            next_level = Some(other);
                        }
                    }
                    CardKind::Node(ni) => {
                        let node = &self.graph.nodes[ni];
                        let tone = node_tone(&node.kind);
                        let is_hit = self.search_hits.contains(&ni);
                        let in_path = self.path_nodes.contains(&node.id);
                        let changed = self.diff_on && self.diff.as_ref().is_some_and(|d| d.changed_node_ids.contains(&node.id));
                        let affected = self.diff_on && self.diff.as_ref().is_some_and(|d| d.affected_node_ids.contains(&node.id));
                        let selected = self.selected_node == Some(ni);
                        let fill = if self.highlight.contains(&node.id) || in_path {
                            pal.hilite
                        } else if is_hit {
                            Color32::from_rgb(255, 236, 170)
                        } else {
                            mix(pal.surface, tone, 0.035)
                        };
                        let border_col = if changed {
                            pal.diff_changed
                        } else if affected {
                            pal.diff_affected
                        } else if selected {
                            pal.accent
                        } else {
                            mix(pal.surface, tone, 0.25)
                        };
                        let border = if selected || changed || affected { 2.0 } else { 1.0 };
                        let s = self.zoom.max(0.5);
                        card_shadow(&painter, rect, 8.0 * s);
                        painter.rect(rect, 8.0 * s, fill, Stroke::new(border, border_col), egui::StrokeKind::Inside);
                        let inner = painter.with_clip_rect(rect.shrink(3.0));
                        inner.text(rect.min + Vec2::new(12.0 * s, 8.0 * s), Align2::LEFT_TOP, truncate(&node.name, 28), FontId::new(13.0 * s, egui::FontFamily::Name("medium".into())), pal.ink);
                        if self.zoom >= 0.45 {
                            inner.text(rect.min + Vec2::new(12.0 * s, 27.0 * s), Align2::LEFT_TOP, &node.kind, FontId::new(11.0 * s, egui::FontFamily::Name("semibold".into())), tone);
                            inner.text(rect.min + Vec2::new(12.0 * s, 46.0 * s), Align2::LEFT_TOP, truncate(&node.summary, 38), FontId::proportional(11.5 * s), pal.muted);
                        }
                        if resp.clicked() {
                            next_node = Some(ni);
                        }
                        if resp.double_clicked() {
                            code_req = Some(ni);
                        }
                        let name = node.name.clone();
                        let kind = node.kind.clone();
                        let summary = node.summary.clone();
                        let has_file = node.file_path.is_some();
                        resp.on_hover_ui(|ui| {
                            ui.strong(&name);
                            ui.label(egui::RichText::new(&kind).color(pal.muted));
                            ui.label(truncate(&summary, 160));
                            if has_file {
                                ui.label(egui::RichText::new("Nhấp đúp để xem mã nguồn").color(pal.muted).small());
                            }
                        });
                    }
                }
            }
            // Kéo thẻ: di chuyển theo chuột (đơn vị thế giới = pixel / zoom); cạnh tự bám theo.
            for (ci, d) in drags {
                let key = match self.cards[ci].kind {
                    CardKind::Layer(i) => (0u8, i),
                    CardKind::Node(i) => (1u8, i),
                    CardKind::Portal(i) => (2u8, i),
                };
                let moved = self.cards[ci].rect.translate(d / self.zoom);
                self.cards[ci].rect = moved;
                self.pos_override.insert(key, moved.min);
                self.fit_pending = 0;
            }
            if let Some(li) = next_level {
                self.enter_layer(li);
            } else if let Some(ni) = next_node {
                self.selected_node = Some(ni);
            }
            if let Some(ni) = code_req {
                self.selected_node = Some(ni);
                self.load_code(ni);
            }

            // Minimap (Minimap.tsx): thu nhỏ toàn bộ thẻ, khung đang xem, bấm để di chuyển.
            if self.minimap && !self.cards.is_empty() {
                if let Some(b) = self.world_bounds() {
                    let mm = Rect::from_min_size(response.rect.max - Vec2::new(190.0, 130.0), Vec2::new(180.0, 120.0));
                    painter.rect(mm, 6.0, pal.surface, Stroke::new(1.0, pal.border), egui::StrokeKind::Inside);
                    let sc = ((mm.width() - 8.0) / b.width()).min((mm.height() - 8.0) / b.height());
                    let to_mm = |p: Pos2| mm.min + Vec2::new(4.0, 4.0) + (p - b.min) * sc;
                    for c in &self.cards {
                        let r = Rect::from_two_pos(to_mm(c.rect.min), to_mm(c.rect.max));
                        let col = match c.kind {
                            CardKind::Layer(li) => hex(LAYER_COLORS[li % LAYER_COLORS.len()]),
                            CardKind::Portal(li) => hex(LAYER_COLORS[li % LAYER_COLORS.len()]),
                            CardKind::Node(_) => pal.muted,
                        };
                        painter.rect_filled(r, 1.0, col);
                    }
                    // Khung nhìn hiện tại trong thế giới
                    let view_w = Rect::from_min_size(Pos2::new(
                        (origin.x - origin.x - self.pan.x) / self.zoom + b.min.x,
                        (origin.y - origin.y - self.pan.y) / self.zoom + b.min.y,
                    ), response.rect.size() / self.zoom);
                    let vr = Rect::from_two_pos(to_mm(view_w.min), to_mm(view_w.max)).intersect(mm);
                    painter.rect_stroke(vr, 0.0, Stroke::new(1.5, pal.accent), egui::StrokeKind::Inside);
                    let mm_resp = ui.interact(mm, Id::new("minimap"), Sense::click_and_drag());
                    if mm_resp.dragged() || mm_resp.clicked() {
                        if let Some(p) = mm_resp.interact_pointer_pos() {
                            let world = b.min + (p - mm.min - Vec2::new(4.0, 4.0)) / sc;
                            self.pan = response.rect.size() / 2.0 - world.to_vec2() * self.zoom;
                            self.fit_pending = 0;
                        }
                    }
                }
            }

            // Overlay diff: chú thích
            if self.diff_on {
                painter.text(origin + Vec2::new(12.0, 30.0), Align2::LEFT_TOP, "Đỏ: thay đổi · Vàng: bị ảnh hưởng", FontId::proportional(12.0), pal.muted);
            }
            let crumb = match self.level {
                Level::Overview => "Tổng quan".to_string(),
                Level::Layer(li) => format!("Tổng quan › {}", self.graph.layers[li].name),
            };
            painter.text(origin + Vec2::new(12.0, 10.0), Align2::LEFT_TOP, crumb, FontId::proportional(13.0), pal.muted);
        });
    }
}

/// Ô chọn node có lọc theo chữ (dùng cho đường đi).
fn pick_node(ui: &mut egui::Ui, salt: &str, names: &[(usize, String)], sel: &mut Option<usize>, filter: &mut String) {
    let label = sel.and_then(|i| names.iter().find(|(j, _)| *j == i)).map(|(_, n)| n.clone()).unwrap_or_else(|| "(chọn)".into());
    egui::ComboBox::from_id_salt(salt).selected_text(truncate(&label, 46)).width(360.0).show_ui(ui, |ui| {
        ui.add(egui::TextEdit::singleline(filter).hint_text("lọc…"));
        let f = filter.to_lowercase();
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (i, n) in names.iter().filter(|(_, n)| f.is_empty() || n.to_lowercase().contains(&f)).take(300) {
                if ui.selectable_label(*sel == Some(*i), n).clicked() {
                    *sel = Some(*i);
                }
            }
        });
    });
}

fn main() -> eframe::Result<()> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".ua/knowledge-graph.json".into());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 620.0])
            .with_position([16.0, 16.0])
            .with_title("Understand — ua-viewer"),
        ..Default::default()
    };
    eframe::run_native(
        "ua-viewer",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(&path)))
        }),
    )
}
