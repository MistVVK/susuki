//! §6.5 GPU analysis path (REQ-DC-006): Vulkan compute via wgpu, shared
//! across NVIDIA/Intel/AMD. Feature-level pixel math only; candidate
//! management, merging and decision stay on the CPU. Any failure (no
//! Vulkan device, init, shader compile, map_async) falls back to the CPU
//! path and the caller records `warnings: analysis_gpu_fallback`.

/// One 16px-wide block row: avgdist results for both frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockStat {
    pub ddist1: i32,
    pub avg1: i32,
    pub ddist2: i32,
    pub avg2: i32,
}

const SHADER: &str = r#"
struct Params { nblocks: u32, lx2: u32, block_height: u32, _pad: u32 };
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> cur: array<u32>;
@group(0) @binding(2) var<storage, read> bef: array<u32>;
@group(0) @binding(3) var<storage, read> offs: array<u32>;
@group(0) @binding(4) var<storage, read_write> outp: array<u32>;

// avgdist of one image block at pixel offset `base` (reference mvec.cpp):
//   d_avg = ((sum + bh*8) / (bh*16)) & 0xFF ; dev = sum |v - d_avg| (u32 wrap)
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let b = gid.x;
    if (b >= p.nblocks) { return; }
    let base = offs[b];
    var sum1: u32 = 0u;
    var sum2: u32 = 0u;
    for (var j: u32 = 0u; j < p.block_height; j = j + 1u) {
        let row = base + j * p.lx2;
        for (var x: u32 = 0u; x < 16u; x = x + 1u) {
            let o = row + x;
            let w = o / 4u;
            let sh = 8u * (o % 4u);
            sum1 = sum1 + ((cur[w] >> sh) & 0xFFu);
            sum2 = sum2 + ((bef[w] >> sh) & 0xFFu);
        }
    }
    let davg1 = ((sum1 + p.block_height * 8u) / (p.block_height * 16u)) & 0xFFu;
    let davg2 = ((sum2 + p.block_height * 8u) / (p.block_height * 16u)) & 0xFFu;
    var dev1: u32 = 0u;
    var dev2: u32 = 0u;
    for (var j: u32 = 0u; j < p.block_height; j = j + 1u) {
        let row = base + j * p.lx2;
        for (var x: u32 = 0u; x < 16u; x = x + 1u) {
            let o = row + x;
            let w = o / 4u;
            let sh = 8u * (o % 4u);
            let v1 = (cur[w] >> sh) & 0xFFu;
            let v2 = (bef[w] >> sh) & 0xFFu;
            dev1 = dev1 + select(v1 - davg1, davg1 - v1, v1 < davg1);
            dev2 = dev2 + select(v2 - davg2, davg2 - v2, v2 < davg2);
        }
    }
    outp[b * 4u + 0u] = dev1;
    outp[b * 4u + 1u] = davg1;
    outp[b * 4u + 2u] = dev2;
    outp[b * 4u + 3u] = davg2;
}
"#;

pub struct GpuEngine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bgl: wgpu::BindGroupLayout,
    logo_pipeline: wgpu::ComputePipeline,
    logo_bgl: wgpu::BindGroupLayout,
}

impl GpuEngine {
    /// Initialize a Vulkan compute device (cross-vendor requirement).
    /// None => CPU path (caller adds `analysis_gpu_fallback` when gpu/auto).
    pub fn init() -> Option<GpuEngine> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok()?;
        if !matches!(adapter.get_info().backend, wgpu::Backend::Vulkan) {
            return None;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("susuki-analysis"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            ..Default::default()
        }))
        .ok()?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("susuki-avgdist"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&layout),
            module: &module,
            entry_point: Some("main"),
            cache: None,
            compilation_options: Default::default(),
        });
        // --- logo pix/areasum pipeline ---
        let lmod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("susuki-logopix"),
            source: wgpu::ShaderSource::Wgsl(LOGO_SHADER.into()),
        });

        let logo_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let logo_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&logo_bgl],
            push_constant_ranges: &[],
        });
        let logo_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&logo_layout),
            module: &lmod,
            entry_point: Some("main"),
            cache: None,
            compilation_options: Default::default(),
        });
        Some(GpuEngine {
            device,
            queue,
            pipeline,
            bgl,
            logo_pipeline,
            logo_bgl,
        })
    }

    /// Per-block avgdist table for the precomputed block offsets.
    /// None on any compute failure (caller falls back to CPU).
    pub fn block_stats(
        &self,
        cur: &[u8],
        bef: &[u8],
        offs: &[u32],
        lx2: u32,
        block_height: u32,
    ) -> Option<Vec<BlockStat>> {
        let n = offs.len() as u32;
        if n == 0 {
            return Some(Vec::new());
        }
        let cur_w = pack_u32(cur);
        let bef_w = pack_u32(bef);
        let params: Vec<u8> = [n, lx2, block_height, 0u32]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let offs_raw: Vec<u8> = offs.iter().flat_map(|v| v.to_le_bytes()).collect();
        let out_bytes = (offs.len() * 4 * 4).next_power_of_two().max(16);

        use wgpu::util::DeviceExt;
        let b_cur = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &cur_w,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let b_bef = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &bef_w,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let b_offs = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &offs_raw,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let b_par = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let b_out = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: out_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (offs.len() * 16) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: b_par.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: b_cur.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: b_bef.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: b_offs.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: b_out.as_entire_binding(),
                },
            ],
        });
        {
            let mut enc = self.device.create_command_encoder(&Default::default());
            {
                let mut cp = enc.begin_compute_pass(&Default::default());
                cp.set_pipeline(&self.pipeline);
                cp.set_bind_group(0, &bg, &[]);
                cp.dispatch_workgroups(n.div_ceil(64), 1, 1);
            }
            enc.copy_buffer_to_buffer(&b_out, 0, &read, 0, (offs.len() * 16) as u64);
            self.queue.submit(Some(enc.finish()));
        }
        let slice = read.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r.is_ok());
        });
        let _ = self.device.poll(wgpu::PollType::Wait);
        rx.recv().ok()?;
        // (the map callback result already flowed through rx; Err => None => CPU)
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity(offs.len());
        for chunk in data.chunks_exact(16) {
            let g = |i: usize| {
                i32::from_le_bytes([
                    chunk[i * 4],
                    chunk[i * 4 + 1],
                    chunk[i * 4 + 2],
                    chunk[i * 4 + 3],
                ])
            };
            out.push(BlockStat {
                ddist1: g(0),
                avg1: g(1),
                ddist2: g(2),
                avg2: g(3),
            });
        }
        drop(data);
        read.unmap();
        Some(out)
    }
}

fn pack_u32(src: &[u8]) -> Vec<u8> {
    // storage buffers need 4-byte multiples; zero pad (never read)
    let mut v = src.to_vec();
    while v.len() % 4 != 0 {
        v.push(0);
    }
    v
}

// ---------------------------------------------------------------------------
// §6.5 LogoCalc 特徴量カーネル（getdif col/row パスのピクセル判定 + areasum）。
// 各スレッド = (pass, i, j) 1 ピクセル。出力は PixEval/AreaEval イベントで、
// Calc への加算適用は CPU（順序非依存のため CPU 全量実行とビット一致）。
// ---------------------------------------------------------------------------

pub const LOGO_THREADS_LAYOUT: u32 = 18; // 7 primary + 7 tail + 4 areasum

/// ワードレイアウト（各イベント 7語）:
/// [0]=mask [1]=hist+1(0=なし) [2]=offedg [3]=sums有効 | off<<16? → 分離
/// [4]=sum_off [5]=sum_on [6]=sum_dif
/// tail-sub 側は mask のみ（exe_type=1 の分岐は cntopp/なし）
pub struct LogoFrameReq<'a> {
    pub data: &'a [u8],
    pub pitch: usize,
    pub lw: u32,
    pub lh: u32,
    /// data 領域先頭 = pos_y*pitch + pos_x（ホスト側でスライス済み可）
    pub parity: u32, // pos_y % 2
    pub dif_y_col: &'a [i32],
    pub dif_y_row: &'a [i32],
    pub dp_y: &'a [i32],
    pub lum16: &'a [i32],
    pub area_y: &'a [i32],
    pub thres_ymax: i32,
    pub thres_yedge: i32,
    pub thres_ydif: i32,
    pub thres_yoffedg: i32,
    pub most_logo_y: i32,
    pub fade_calcstep: i64,
}

const LOGO_SHADER: &str = r#"
// 1: mask, 2: hist(+1), 3: offedg, 4: sumflag, 5: sumoff, 6: sumon, 7: sumdif
struct Lp {
    lw: u32, lh: u32, pitch: u32, parity: u32,
    thres_ymax: i32, thres_yedge: i32, thres_ydif: i32, thres_yoffedg: i32,
    most_logo_y: i32, fade_calcstep: i32,
};
@group(0) @binding(0) var<uniform> q: Lp;
struct Blk { px0: u32, dcol: u32, drow: u32, dpy: u32, lum: u32, ar: u32, pad: u32, n: u32 };
@group(0) @binding(1) var<uniform> blk: Blk;
@group(0) @binding(2) var<storage, read> sd: array<u32>;   // 共有ストレージ（px:u32配列, 以下 i32）
@group(0) @binding(3) var<storage, read_write> ev: array<u32>;

fn sd_px(o: u32) -> u32 { return sd[blk.px0 + o / 4u]; }
fn pxb(o: u32) -> u32 { return (sd_px(o) >> (8u * (o % 4u))) & 0xFFu; }
fn v_dcol(o: u32) -> i32 { return i32(sd[blk.dcol + o]); }
fn v_drow(o: u32) -> i32 { return i32(sd[blk.drow + o]); }
fn v_dpy(o: u32) -> i32 { return i32(sd[blk.dpy + o]); }
fn v_lum(o: u32) -> i32 { return i32(sd[blk.lum + o]); }
fn v_ar(o: u32) -> i32 { return i32(sd[blk.ar + o]); }

// ConvInvLogo(d, dp, y) = (d*1000 - y*dp + (1000-dp)/2) / (1000-dp)
fn conv_inv(d: i32, dp: i32, y: i32) -> i32 {
    return (d * 1000 - y * dp + (1000 - dp) / 2) / (1000 - dp);
}
// ConvLogo(d, dp, y) = (d*(1000-dp) + y*dp + 500) / 1000
fn conv(d: i32, dp: i32, y: i32) -> i32 {
    return (d * (1000 - dp) + y * dp + 500) / 1000;
}

fn ab(x: i32) -> i32 { return select(-x, x, x >= 0); }

// pix decision core → mask/hist/sums; 戻り値: tail-call フラグ
fn z32(v: i32) -> u32 { return bitcast<u32>(v); }
fn core(d0v: u32, d1v: u32, dp0: i32, y0v: i32, dp1: i32, y1v: i32,
        exet: i32, base: u32) -> u32 {  // base: このスレッド用イベント語先頭
    let d0_src = i32(d0v) * 16;
    var d0_rev = conv_inv(d0_src, dp0, y0v);
    let d1_rev0 = conv(d0_rev, dp1, y1v);
    d0_rev = clamp(d0_rev, 0, 255 * 16);
    var d1_rev = clamp(d1_rev0, 0, 255 * 16);
    var d1 = i32(d1v);
    var d1s = i32(d0v);
    var d1r = (d1_rev + 8) >> 4;
    if (d1 > q.thres_ymax) { d1 = q.thres_ymax; }
    if (d1s > q.thres_ymax) { d1s = q.thres_ymax; }
    if (d1r > q.thres_ymax) { d1r = q.thres_ymax; }
    let oppside = i32((select(0, 1, (d1 < d1s && d1s < d1r) || (d1 > d1s && d1s > d1r))));
    let val_offedg = i32(select(0, 1, i32(d0v) > q.thres_yoffedg || i32(d1v) > q.thres_yoffedg));
    let d1s_fine = d1s * 16;
    var d1rfine = d1_rev;
    if (d1rfine > q.thres_ymax * 16) { d1rfine = q.thres_ymax * 16; }
    let d1dif_fine = d1rfine - d1s_fine;
    let d0calc_adif = ab(d0_src - d0_rev);
    let d1calc_adif = ab(d0_rev - d1_rev);
    let d0calc_much = i32(select(0, 1, d0calc_adif * 2 >= d1calc_adif && d0calc_adif >= 16));
    let d1calc_much = i32(select(0, 1, d1calc_adif * 2 >= d0calc_adif && d1calc_adif >= 16));
    let most_adif0 = ab(q.most_logo_y - ((y0v + 8) >> 4));
    let most_adif1 = ab(q.most_logo_y - ((y1v + 8) >> 4));
    let d1dif = d1r - d1s;
    var chkopp = 0i;

    var mask = 0u; var hist = 0i; var offedg = 0i;
    var sumf = 0i; var soff = 0i; var son = 0i; var sdif = 0i;

    if ((i32(d0v) > q.thres_ymax && i32(d1v) > q.thres_ymax && d1r >= q.thres_ymax - 1)
        || (i32(d0v) > q.thres_yedge && i32(d1v) > q.thres_yedge && d1r > q.thres_yedge - 10)
        || (ab(d1dif) < 10 && ((d0calc_adif >= 16 && most_adif0 > 2) || (d1calc_adif >= 16 && most_adif1 > 2)))) {
        // skip
    } else if (((ab(d1dif) < 14 || dp0 < 100) && d0calc_much > 0 && most_adif0 > 32)
            || ((ab(d1dif) < 14 || dp1 < 100) && d1calc_much > 0 && most_adif1 > 32)) {
        if (exet == 0 && q.thres_yoffedg >= 255) {
            if (ab(d1 - d1s) < 4) { mask = 1u; }        // CNTS_OFF
            else if (ab(d1 - d1r) < 5) { mask = 2u; }   // CNTS_ON
        }
    } else if (ab(d1 - d1s) < 4 && q.thres_yoffedg < 255
            && ((dp0 < 100 && d0calc_adif >= 16 && most_adif0 > 32)
             || (dp1 < 100 && d0calc_adif >= 16 && most_adif1 > 32))) {
        // skip (nologo)
    } else if (ab(d1dif) >= 10) {
        if (exet == 0) {
            let val = ((d1 - d1s) * q.fade_calcstep + d1dif / 2) / d1dif + 2;
            if (val >= 0 && val < 36) { hist = val; }
            if (ab(d1 - d1s) < 4) { mask = 4u; offedg = val_offedg; }      // CNT_OFF
            else if (ab(d1 - d1r) < 5) { mask = 8u; }                       // CNT_ON
            else { mask = 16u; if (exet == 0 && oppside == 1) { chkopp = 1; } }   // CNT_MV
        }
    } else {
        if (ab(d1dif) >= 6) {
            if (exet == 0) {
                if (ab(d1 - d1s) < 3) { mask = 32u; offedg = val_offedg; } // CNTF_OFF
                else if (ab(d1 - d1r) < 3) { mask = 64u; }                 // CNTF_ON
                else if (exet == 0 && oppside == 1) { chkopp = 1; }
            } else if (exet == 1 && ab(d1 - d1r) < 3) { mask = 256u; }     // CNTOPP_ON
        } else if (ab(d1dif_fine) >= q.thres_ydif && ab(d1dif_fine) >= 56) {
            if (exet == 0) {
                if (ab(d1 - d1s) <= 0 && ab(d1 - d1r) >= 3) { mask = 32u; offedg = val_offedg; }
                else if (ab(d1 - d1s) >= 2 && ab(d1 - d1r) <= 3) { mask = 64u; }
                else if (exet == 0 && oppside == 1) { chkopp = 1; }
            } else if (exet == 1) {
                if (ab(d1 - d1s) <= 0 && ab(d1 - d1r) >= 3) {}
                else if (ab(d1 - d1s) >= 2 && ab(d1 - d1r) <= 3) { mask = 256u; }
            }
        } else if (ab(d1dif_fine) >= q.thres_ydif && q.thres_ydif > 0) {
            if (exet == 0) {
                if (ab(d1 - d1s) <= 0) { mask = 32u; offedg = val_offedg; }
                else if (ab(d1 - d1r) <= 1) { mask = 64u; }
                else if (exet == 0 && oppside == 1) { chkopp = 1; }
            } else if (exet == 1) {
                if (ab(d1 - d1s) <= 0) {}
                else if (ab(d1 - d1r) <= 1) { mask = 256u; }
            }
        } else if (exet == 0) {
            mask = 128u;                                                    // CNTF_ST
            if (exet == 0 && oppside == 1) { chkopp = 1; }
        }
        if (exet == 0) {
            if (dp0 < dp1) {
                if (d1_rev - d0_src >= 48 && d1 > d1s - 4 && d1 < d1r + 4) {
                    sumf = 1; soff = i32(d0v); son = i32(d1v); sdif = d1r - d1s;
                }
            } else if (dp0 > dp1 && d0_src - d1_rev >= 48 && d1 > d1r - 4 && d1 < d1s + 4) {
                sumf = 1; soff = i32(d1v); son = i32(d0v); sdif = d1s - d1r;
            }
        }
    }
    ev[base + 0u] = mask; ev[base + 1u] = bitcast<u32>(hist) + 1u; ev[base + 2u] = bitcast<u32>(offedg);
    ev[base + 3u] = bitcast<u32>(sumf); ev[base + 4u] = bitcast<u32>(soff);
    ev[base + 5u] = bitcast<u32>(son); ev[base + 6u] = bitcast<u32>(sdif);
    return bitcast<u32>(chkopp);
}

// areasum → mode/bits/data/cal を mask に詰めて2語で返す（ev b0+7..9）
fn asum(d0v: u32, dp0: i32, y0v: i32, ar: i32, base: u32) {  // 語14..16 に書込
    var mode = 0u; var bits = 0u; var cal = 0u;
    if ((ar & 0x3) == 1) { mode = 1u; bits = bitcast<u32>(ar >> 8); }
    else if ((ar & 0x3) == 2) {
        mode = 2u; bits = bitcast<u32>(ar >> 8);
        var d0_rev = conv_inv(i32(d0v) * 16, dp0, y0v);
        d0_rev = clamp(d0_rev, 16, 254 * 16);
        var d0r = (d0_rev + 8) / 16;
        if (i32(d0v) > q.thres_ymax && i32(d0v) > q.most_logo_y) { d0r = i32(d0v); }
        cal = bitcast<u32>(d0r);
    }
    ev[base + 14u] = mode; ev[base + 15u] = bits | (d0v << 16); ev[base + 16u] = cal;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let kpass = gid.z;                 // 0=col, 1=row
    let i = gid.y;                    // region row
    let j = gid.x;                    // region col
    let lw = q.lw; let lh = q.lh;
    if (j >= lw || i >= lh) { return; }
    let slot = ((kpass * lh + i) * lw + j) * 18u;
    // ガードで return するスレッドもゼロで初期化する（apply 側が読むため）
    for (var k: u32 = 0u; k < 18u; k = k + 1u) { ev[slot + k] = 0u; }
    let idx0 = i * lw + j;
    let row = i * q.pitch + j;
    if (kpass == 0u) {
        // 原典 col_pass: core と areasum は独立ガード
        if (v_dcol(idx0) > 0) {
            let d0 = pxb(row);
            let d1 = pxb(row + 2u);
            let idx1 = idx0 + 2u;
            let chk = core(d0, d1, v_dpy(idx0), v_lum(idx0), v_dpy(idx1), v_lum(idx1), 0, slot);
            if (chk != 0u) {
                // tail recursion（data0/data1 swap, dp/y pair は維持、thres=100 quirk）
                core(d1, d0, v_dpy(idx0), v_lum(idx0), v_dpy(idx1), v_lum(idx1), 1, slot + 7u);
            }
        }
        if (v_ar(idx0) > 0) {
            asum(pxb(row), v_dpy(idx0), v_lum(idx0), v_ar(idx0), slot);
        }
    } else {
        if (v_drow(idx0) <= 0 || i + 2u >= lh) { return; }
        let d0 = pxb(row);
        let d1 = pxb(row + 2u * q.pitch);
        let idx1 = idx0 + 2u * lw;
        let chk = core(d0, d1, v_dpy(idx0), v_lum(idx0), v_dpy(idx1), v_lum(idx1), 0, slot);
        if (chk != 0u) {
            core(d1, d0, v_dpy(idx0), v_lum(idx0), v_dpy(idx1), v_lum(idx1), 1, slot + 7u);
        }
    }
}
"#;

impl GpuEngine {
    /// §6.5: LogoCalc pix/areasum イベントを GPU で生成する。
    /// None は計算失敗（CPU フォールバック指示）。
    pub fn logo_frame_events(&self, req: &LogoFrameReq) -> Option<Vec<u32>> {
        let threads = 2usize * (req.lw as usize) * (req.lh as usize);
        if threads == 0 {
            return Some(Vec::new());
        }
        let words = threads * 18;
        let npix = (req.lw * req.lh) as usize;
        if req.dif_y_col.len() < npix
            || req.dif_y_row.len() < npix
            || req.dp_y.len() < npix
            || req.lum16.len() < npix
            || req.area_y.len() < npix
        {
            return None;
        }
        // shared storage: [px u32][dcol][drow][dpy][lum][ar]（各 4-byte word）
        // 末尾行の tail アクセス (row+2*pitch, +2px) に耐える語数へ余裕を持たせる
        let need_bytes = req.data.len() + 3 * req.pitch + (req.lw as usize) + 16;
        let mut padded = req.data.to_vec();
        padded.resize(need_bytes + 3, 0);
        let mut sd: Vec<u32> = Vec::new();
        let off_px = 0u32;
        {
            let v = padded;
            for c in v.chunks_exact(4) {
                sd.push(u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
            }
        }
        let push_arr = |sd: &mut Vec<u32>, arr: &[i32]| -> u32 {
            let off = sd.len() as u32;
            sd.extend(arr.iter().map(|&x| x as u32));
            off
        };
        let off_col = push_arr(&mut sd, req.dif_y_col);
        let off_row = push_arr(&mut sd, req.dif_y_row);
        let off_dpy = push_arr(&mut sd, req.dp_y);
        let off_lum = push_arr(&mut sd, req.lum16);
        let off_ar = push_arr(&mut sd, req.area_y);
        let sd_bytes: Vec<u8> = sd.iter().flat_map(|v| v.to_le_bytes()).collect();
        let params: Vec<u8> = {
            let mut v = Vec::new();
            for x in [
                req.lw,
                req.lh,
                req.pitch as u32,
                req.parity,
                req.thres_ymax as u32,
                req.thres_yedge as u32,
                req.thres_ydif as u32,
                req.thres_yoffedg as u32,
                req.most_logo_y as u32,
                req.fade_calcstep as i32 as u32,
                0u32,
                0u32,
            ] {
                v.extend_from_slice(&x.to_le_bytes());
            }
            v
        };
        let blk: Vec<u8> = {
            let mut v = Vec::new();
            for x in [
                off_px,
                off_col,
                off_row,
                off_dpy,
                off_lum,
                off_ar,
                0u32,
                sd.len() as u32,
            ] {
                v.extend_from_slice(&x.to_le_bytes());
            }
            v
        };
        use wgpu::util::DeviceExt;
        let b_par = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let b_blk = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &blk,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let b_sd = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &sd_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let out_words = words.next_power_of_two().max(64);
        let b_ev = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (out_words * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (words * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.logo_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: b_par.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: b_blk.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: b_sd.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: b_ev.as_entire_binding(),
                },
            ],
        });
        {
            let mut enc = self.device.create_command_encoder(&Default::default());
            {
                let mut cp = enc.begin_compute_pass(&Default::default());
                cp.set_pipeline(&self.logo_pipeline);
                cp.set_bind_group(0, &bg, &[]);
                cp.dispatch_workgroups(req.lw.div_ceil(64), req.lh, 2);
            }
            enc.copy_buffer_to_buffer(&b_ev, 0, &read, 0, (words * 4) as u64);
            self.queue.submit(Some(enc.finish()));
        }
        let slice = read.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r.is_ok());
        });
        let _ = self.device.poll(wgpu::PollType::Wait);
        rx.recv().ok()?;
        let data = slice.get_mapped_range();
        let mut outv = vec![0u32; words];
        for (i, ch) in data.chunks_exact(4).enumerate() {
            if i < words {
                outv[i] = u32::from_le_bytes([ch[0], ch[1], ch[2], ch[3]]);
            }
        }
        drop(data);
        Some(outv)
    }
}

/// GPU イベント表を Calc 1/2 へ適用（apply_pix_eval / apply_area_eval と同一
/// の書き込みを CPU で順不同で実施 — 加算のみなので結果は一意）。
pub fn apply_logo_events(
    c1: &mut crate::logo_calc::Calc,
    c2: &mut crate::logo_calc::Calc,
    ev: &[u32],
    lw: u32,
    lh: u32,
    parity: u32,
) {
    apply_logo_group(c1, ev, lw, lh, parity, 0);
    apply_logo_group(c2, ev, lw, lh, parity, 1);
}

fn apply_logo_group(
    c: &mut crate::logo_calc::Calc,
    ev: &[u32],
    lw: u32,
    lh: u32,
    parity: u32,
    want: u32,
) {
    use crate::logo_calc::{apply_area_eval, apply_pix_eval, AreaEval, PixEval};
    let g = |i: usize| -> i32 { ev[i] as i32 };
    for pass in 0..2u32 {
        for row in 0..lh {
            if (parity + row) % 2 != want {
                continue;
            }
            for col in 0..lw {
                let slot = (((pass * lh + row) * lw + col) as usize) * 18;
                if slot + 18 > ev.len() {
                    return;
                }
                apply_pix_eval(
                    c,
                    &PixEval {
                        mask: ev[slot],
                        hist: g(slot + 1) - 1,
                        offedg: g(slot + 2) as i64,
                        sum_valid: g(slot + 3) != 0,
                        sum_off: g(slot + 4) as i64,
                        sum_on: g(slot + 5) as i64,
                        sum_dif: g(slot + 6) as i64,
                    },
                );
                apply_pix_eval(
                    c,
                    &PixEval {
                        mask: ev[slot + 7],
                        hist: g(slot + 8) - 1,
                        offedg: g(slot + 9) as i64,
                        sum_valid: false,
                        sum_off: 0,
                        sum_on: 0,
                        sum_dif: 0,
                    },
                );
                if pass == 0 {
                    let mode = ev[slot + 14];
                    if mode != 0 {
                        let bits_data = ev[slot + 15];
                        apply_area_eval(
                            c,
                            &AreaEval {
                                mode,
                                bits: bits_data & 0xFFFF,
                                data0: bits_data >> 16,
                                cal: ev[slot + 16],
                            },
                        );
                    }
                }
            }
        }
    }
}
