//! Port logic bố cục từ plugin gốc:
//! - `dashboard/src/utils/force-layout.ts` (computeForceLayout): mô phỏng lực kiểu d3-force
//!   với các lực link, charge (many-body), center, collide, và lực kéo về cụm theo layer.
//! - `dashboard/src/components/KnowledgeGraphView.tsx` (getNodeDimensions): kích thước node
//!   tăng theo số cạnh.
//!
//! Các hằng số giữ nguyên giá trị của plugin gốc. Thuật toán tái hiện d3-force ở mức
//! công thức (không sao chép mã), nên vị trí có thể lệch nhẹ so với bản web.

pub const NODE_WIDTH: f32 = 280.0;
pub const NODE_HEIGHT: f32 = 120.0;

/// Kích thước node: scale = clamp(0.85 + 0.03·số_cạnh, 0.85, 1.5).
pub fn node_dims(edge_count: usize) -> (f32, f32) {
    let scale = (0.85 + edge_count as f32 * 0.03).clamp(0.85, 1.5);
    ((NODE_WIDTH * scale).round(), (NODE_HEIGHT * scale).round())
}

pub struct Input<'a> {
    pub ids: &'a [String],
    pub dims: &'a [(f32, f32)],
    /// Chỉ số layer của node (None nếu không thuộc layer nào).
    pub community: &'a [Option<usize>],
    /// Cặp chỉ số (source, target) của các cạnh.
    pub links: &'a [(usize, usize)],
}

/// Trả về toạ độ góc trên-trái của từng node (world space), theo thứ tự `ids`.
pub fn force_layout(inp: &Input) -> Vec<(f32, f32)> {
    let n = inp.ids.len();
    if n == 0 {
        return Vec::new();
    }

    // Vị trí khởi tạo: phyllotaxis như d3-force (bán kính 10·√(0.5+i), góc i·golden angle).
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    let mut x: Vec<f32> = (0..n).map(|i| {
        let r = 10.0 * (0.5 + i as f32).sqrt();
        r * (i as f32 * golden).cos()
    }).collect();
    let mut y: Vec<f32> = (0..n).map(|i| {
        let r = 10.0 * (0.5 + i as f32).sqrt();
        r * (i as f32 * golden).sin()
    }).collect();
    let mut vx = vec![0.0f32; n];
    let mut vy = vec![0.0f32; n];

    // Bậc của node (dùng cho bias của lực link, như d3).
    let mut degree = vec![0usize; n];
    for &(s, t) in inp.links {
        degree[s] += 1;
        degree[t] += 1;
    }

    // Cụm: các layer xuất hiện, sắp xếp và đặt đều trên vòng tròn.
    let mut comms: Vec<usize> = inp.community.iter().flatten().copied().collect();
    comms.sort_unstable();
    comms.dedup();
    let comm_count = comms.len().max(1);
    let cluster_radius = (600f32).max(n as f32 * 5.0);
    let comm_angle = |c: usize| -> f32 {
        let idx = comms.iter().position(|&v| v == c).unwrap_or(0);
        2.0 * std::f32::consts::PI * idx as f32 / comm_count as f32
    };
    let targets: Vec<Option<(f32, f32)>> = inp
        .community
        .iter()
        .map(|c| c.map(|c| {
            let a = comm_angle(c);
            (a.cos() * cluster_radius, a.sin() * cluster_radius)
        }))
        .collect();

    let is_large = n > 100;
    let charge = if is_large { -600.0f32 } else { -350.0 };
    let link_distance = if is_large { 250.0f32 } else { 150.0 };
    let radii: Vec<f32> = inp
        .dims
        .iter()
        .map(|(w, _)| ((w + 40.0) / 2.0).max(20.0))
        .collect();

    let ticks = n.clamp(100, 300);
    let mut alpha = 1.0f32;
    let alpha_min = 0.001f32;
    let alpha_decay = 1.0 - alpha_min.powf(1.0 / 300.0);
    let velocity_decay = 0.6f32; // d3 mặc định: v *= (1 − 0.6)
    let dist_max_sq = 1500.0f32 * 1500.0;

    for _ in 0..ticks {
        alpha += (0.0 - alpha) * alpha_decay;

        // 1. Link: kéo/đẩy các cạnh về khoảng cách link_distance, strength 0.2.
        for &(s, t) in inp.links {
            let mut dx = x[t] + vx[t] - x[s] - vx[s];
            let mut dy = y[t] + vy[t] - y[s] - vy[s];
            if dx == 0.0 {
                dx = 1e-6;
            }
            if dy == 0.0 {
                dy = 1e-6;
            }
            let l = (dx * dx + dy * dy).sqrt();
            let k = (l - link_distance) / l * alpha * 0.2;
            dx *= k;
            dy *= k;
            let bias_t = degree[s] as f32 / (degree[s] + degree[t]).max(1) as f32;
            vx[t] -= dx * bias_t;
            vy[t] -= dy * bias_t;
            vx[s] += dx * (1.0 - bias_t);
            vy[s] += dy * (1.0 - bias_t);
        }

        // 2. Charge (many-body): đẩy nhau, cắt ở distanceMax 1500.
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = x[j] - x[i];
                let dy = y[j] - y[i];
                let mut d2 = dx * dx + dy * dy;
                if d2 == 0.0 {
                    d2 = 1.0;
                }
                if d2 >= dist_max_sq {
                    continue;
                }
                let w = charge * alpha / d2;
                vx[i] -= dx * w;
                vy[i] -= dy * w;
                vx[j] += dx * w;
                vy[j] += dy * w;
            }
        }

        // 3. Lực kéo về cụm (forceX/forceY, strength 0.3).
        for i in 0..n {
            if let Some((tx, ty)) = targets[i] {
                vx[i] += (tx - x[i]) * 0.3 * alpha;
                vy[i] += (ty - y[i]) * 0.3 * alpha;
            }
        }

        // 4. Collide: đẩy các node chồng lấp, strength 0.8.
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = x[i] + vx[i] - x[j] - vx[j];
                let dy = y[i] + vy[i] - y[j] - vy[j];
                let l2 = dx * dx + dy * dy;
                let r = radii[i] + radii[j];
                if l2 < r * r {
                    let mut l = l2.sqrt();
                    if l == 0.0 {
                        l = 1e-6;
                    }
                    let amount = (r - l) / l * 0.8;
                    let wi = radii[j] * radii[j] / (radii[i] * radii[i] + radii[j] * radii[j]);
                    vx[i] += dx * amount * wi;
                    vy[i] += dy * amount * wi;
                    vx[j] -= dx * amount * (1.0 - wi);
                    vy[j] -= dy * amount * (1.0 - wi);
                }
            }
        }

        // 5. Center: kéo trọng tâm về gốc, strength 0.03.
        let (mut sx, mut sy) = (0.0f32, 0.0f32);
        for i in 0..n {
            sx += x[i];
            sy += y[i];
        }
        sx /= n as f32;
        sy /= n as f32;
        for i in 0..n {
            x[i] -= sx * 0.03;
            y[i] -= sy * 0.03;
        }

        // 6. Tích hợp vị trí.
        for i in 0..n {
            vx[i] *= 1.0 - velocity_decay;
            vy[i] *= 1.0 - velocity_decay;
            x[i] += vx[i];
            y[i] += vy[i];
        }
    }

    // Góc trên-trái = tâm − (w/2, h/2), như plugin gốc.
    (0..n)
        .map(|i| {
            let (w, h) = inp.dims[i];
            let (cx, cy) = (x[i], y[i]);
            (cx - w / 2.0, cy - h / 2.0)
        })
        .collect()
}
