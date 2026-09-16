//! 终端渲染:tokei 式布局(语言分类 + |- 测试子行 + 文件列),
//! 叠加彩色数字与实心比例条。
//!
//! 配色:增/删/改 = 绿/浅红/蓝(#3fb950/#ff9492/#58a6ff,红绿轴 CVD
//! ΔE 9.3 通过;绿↔蓝 tritan 落在允许区)。比例条为纯色实心段 +
//! 暗色轨道,段序固定且数字全部直标——不依赖颜色也能读。
//! 能力检测三级:COLORTERM 真彩 → TERM 含 256color 用 256 色近似 →
//! 否则 8 色;尊重 NO_COLOR。

use std::fmt::Write as _;

use crate::classify::Cat;
use crate::collect::{Dim, LangStat, Report};

const LABEL_W: usize = 14; // 语言列
const FILE_W: usize = 6; // 文件数列
const NUM_W: usize = 10; // 新增/删除/修改列
const BAR_W: usize = 18; // 比例条列
const WIDTH: usize = 1 + LABEL_W + FILE_W + NUM_W * 3 + 2 + BAR_W;

#[derive(Clone, Copy)]
pub enum Ink {
    Plain,
    Head,  // 粗体
    Faint, // 弱化(分隔线/子行/合计)
    Track, // 比例条轨道,比 Faint 更暗
    Title, // 标题强调色
    Add,
    Del,
    Mod,
}

#[derive(Clone, Copy, PartialEq)]
enum ColorMode {
    Off,
    Ansi8,
    Ansi256,
    True,
}

pub struct Palette {
    mode: ColorMode,
}

impl Palette {
    pub fn new(on: bool) -> Self {
        let mode = if !on {
            ColorMode::Off
        } else if std::env::var("COLORTERM")
            .map(|v| v.contains("truecolor") || v.contains("24bit"))
            .unwrap_or(false)
        {
            ColorMode::True
        } else if std::env::var("TERM")
            .map(|v| v.contains("256color"))
            .unwrap_or(false)
        {
            ColorMode::Ansi256
        } else {
            ColorMode::Ansi8
        };
        Self { mode }
    }

    fn paint(&self, ink: Ink, s: &str) -> String {
        let code: &str = match (self.mode, ink) {
            (ColorMode::Off, _) | (_, Ink::Plain) => return s.to_string(),
            (_, Ink::Head) => "1",
            (ColorMode::True, Ink::Faint) => "38;2;110;118;129",
            (ColorMode::True, Ink::Track) => "38;2;48;54;61",
            (ColorMode::Ansi256, Ink::Faint) => "38;5;243",
            (ColorMode::Ansi256, Ink::Track) => "38;5;236",
            (ColorMode::Ansi256, Ink::Title) => "1;38;5;80",
            (ColorMode::Ansi256, Ink::Add) => "38;5;77",
            (ColorMode::Ansi256, Ink::Del) => "38;5;210",
            (ColorMode::Ansi256, Ink::Mod) => "38;5;75",
            (ColorMode::Ansi8, Ink::Track) => "2",
            (ColorMode::True, Ink::Title) => "1;38;2;86;212;221",
            (ColorMode::True, Ink::Add) => "38;2;63;185;80",
            (ColorMode::True, Ink::Del) => "38;2;255;148;146",
            (ColorMode::True, Ink::Mod) => "38;2;88;166;255",
            (ColorMode::Ansi8, Ink::Faint) => "2",
            (ColorMode::Ansi8, Ink::Title) => "1;36",
            (ColorMode::Ansi8, Ink::Add) => "32",
            (ColorMode::Ansi8, Ink::Del) => "31",
            (ColorMode::Ansi8, Ink::Mod) => "34",
        };
        format!("\x1b[{code}m{s}\x1b[0m")
    }
}

/// 终端显示宽度,CJK 字符占 2 列。
fn disp_len(s: &str) -> usize {
    s.chars()
        .map(|r| if (r as u32) > 0x2E7F { 2 } else { 1 })
        .sum()
}

fn pad_right(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(disp_len(s))))
}

fn pad_left(s: &str, w: usize) -> String {
    format!("{}{s}", " ".repeat(w.saturating_sub(disp_len(s))))
}

type Cells = Vec<(String, Ink)>;

fn emit(b: &mut String, c: &Palette, cells: Cells) {
    let body: String = cells.iter().map(|(t, i)| c.paint(*i, t)).collect();
    let _ = writeln!(b, "{}", body.trim_end());
}

fn heavy(b: &mut String, c: &Palette) {
    let _ = writeln!(b, "{}", c.paint(Ink::Faint, &"━".repeat(WIDTH)));
}

fn light(b: &mut String, c: &Palette) {
    let _ = writeln!(b, "{}", c.paint(Ink::Faint, &"─".repeat(WIDTH)));
}

/// 数字格:0 → 空白;行数带 +/-/~ 前缀,文件个数不带。
fn num_cells(d: &Dim, prefixed: bool) -> Cells {
    let (pa, pd, pm) = if prefixed {
        ("+", "-", "~")
    } else {
        ("", "", "")
    };
    let mut out = Vec::new();
    for (val, pre, ink) in [
        (d.added, pa, Ink::Add),
        (d.deleted, pd, Ink::Del),
        (d.modified, pm, Ink::Mod),
    ] {
        if val == 0 {
            out.push((" ".repeat(NUM_W), Ink::Plain));
        } else {
            out.push((pad_left(&format!("{pre}{val}"), NUM_W), ink));
        }
    }
    out
}

/// 堆叠比例条:纯色实心段(绿新增/红删除/蓝修改)+ 暗色轨道铺满到定宽,
/// 段长 ∝ 变更量(跨行可比)。段序固定且数字直标,不依赖颜色也能读。
fn bar_cells(d: &Dim, max_total: usize) -> Cells {
    let total = d.added + d.deleted + d.modified;
    if total == 0 || max_total == 0 {
        return vec![(String::new(), Ink::Plain)];
    }
    let vals = [d.added, d.deleted, d.modified];
    let nonzero = vals.iter().filter(|v| **v > 0).count();
    let len = ((total as f64 / max_total as f64) * BAR_W as f64).round() as usize;
    let len = len.clamp(nonzero, BAR_W);

    // 按占比分配段长,非零段至少 1 格,总长精确等于 len
    let mut seg = [0usize; 3];
    let mut assigned = 0;
    for (i, v) in vals.iter().enumerate() {
        if *v > 0 {
            seg[i] = (((*v as f64 / total as f64) * len as f64).floor() as usize).max(1);
            assigned += seg[i];
        }
    }
    while assigned > len {
        let i = (0..3).max_by_key(|&i| seg[i]).expect("固定 3 段");
        if seg[i] <= 1 {
            break;
        }
        seg[i] -= 1;
        assigned -= 1;
    }
    while assigned < len {
        let i = (0..3)
            .filter(|&i| vals[i] > 0)
            .max_by_key(|&i| vals[i])
            .expect("total>0 必有非零段");
        seg[i] += 1;
        assigned += 1;
    }

    vec![
        ("█".repeat(seg[0]), Ink::Add),
        ("█".repeat(seg[1]), Ink::Del),
        ("█".repeat(seg[2]), Ink::Mod),
        ("░".repeat(BAR_W - len), Ink::Track),
    ]
}

fn row(
    label: &str,
    ink: Ink,
    files: Option<usize>,
    d: &Dim,
    max_total: usize,
    prefixed: bool,
) -> Cells {
    let mut cells: Cells = vec![(format!(" {}", pad_right(label, LABEL_W)), ink)];
    match files {
        Some(n) if n > 0 => cells.push((pad_left(&n.to_string(), FILE_W), Ink::Plain)),
        _ => cells.push((" ".repeat(FILE_W), Ink::Plain)),
    }
    cells.extend(num_cells(d, prefixed));
    if max_total > 0 {
        cells.push(("  ".to_string(), Ink::Plain));
        cells.extend(bar_cells(d, max_total));
    }
    cells
}

fn sum_dim(a: &Dim, b: &Dim) -> Dim {
    Dim {
        added: a.added + b.added,
        deleted: a.deleted + b.deleted,
        modified: a.modified + b.modified,
    }
}

pub fn render(rep: &Report, color: bool, verbose: bool) -> String {
    let c = Palette::new(color);
    let mut b = String::new();
    let empty = LangStat::default();

    // 标题行
    let mut meta = Vec::new();
    if rep.commits > 0 {
        meta.push(format!("{} 个提交", rep.commits));
    }
    let n_files = rep.files.added + rep.files.deleted + rep.files.modified;
    meta.push(format!("{n_files} 个文件变更"));
    let meta = meta.join(" · ");
    let title = format!(" Numeria · {}", rep.range_spec);
    let gap = WIDTH.saturating_sub(disp_len(&title) + disp_len(&meta));
    emit(
        &mut b,
        &c,
        vec![
            (title, Ink::Title),
            (" ".repeat(gap), Ink::Plain),
            (meta, Ink::Faint),
        ],
    );

    heavy(&mut b, &c);
    emit(
        &mut b,
        &c,
        vec![
            (format!(" {}", pad_right("语言", LABEL_W)), Ink::Head),
            (pad_left("文件", FILE_W), Ink::Head),
            (pad_left("新增", NUM_W), Ink::Head),
            (pad_left("删除", NUM_W), Ink::Head),
            (pad_left("修改", NUM_W), Ink::Head),
        ],
    );
    heavy(&mut b, &c);

    // 比例尺:所有将展示的行里的最大变更量
    let lang = |cat: Cat| rep.by_lang.get(&cat).copied().unwrap_or(empty);
    let shown = [
        lang(Cat::GoCode).lines,
        lang(Cat::GoTest).lines,
        lang(Cat::PyCode).lines,
        lang(Cat::PyTest).lines,
        lang(Cat::Other).lines,
    ];
    let max_total = shown
        .iter()
        .map(|d| d.added + d.deleted + d.modified)
        .max()
        .unwrap_or(0);

    // 语言分组:主行 + |- 测试 子行 + (合计)
    let mut groups: Vec<Vec<Cells>> = Vec::new();
    for (name, code_cat, test_cat) in [
        ("Go", Cat::GoCode, Cat::GoTest),
        ("Python", Cat::PyCode, Cat::PyTest),
    ] {
        let code = lang(code_cat);
        let test = lang(test_cat);
        let has_code = code.files > 0 || !code.lines.is_empty();
        let has_test = test.files > 0 || !test.lines.is_empty();
        let mut g = Vec::new();
        match (has_code, has_test) {
            (true, false) => {
                g.push(row(
                    name,
                    Ink::Plain,
                    Some(code.files),
                    &code.lines,
                    max_total,
                    true,
                ));
            }
            (true, true) => {
                g.push(row(
                    name,
                    Ink::Plain,
                    Some(code.files),
                    &code.lines,
                    max_total,
                    true,
                ));
                g.push(row(
                    "|- 测试",
                    Ink::Faint,
                    Some(test.files),
                    &test.lines,
                    max_total,
                    true,
                ));
                g.push(row(
                    "(合计)",
                    Ink::Faint,
                    Some(code.files + test.files),
                    &sum_dim(&code.lines, &test.lines),
                    0,
                    true,
                ));
            }
            (false, true) => {
                g.push(row(
                    &format!("{name} 测试"),
                    Ink::Plain,
                    Some(test.files),
                    &test.lines,
                    max_total,
                    true,
                ));
            }
            (false, false) => {}
        }
        if !g.is_empty() {
            groups.push(g);
        }
    }
    let other = lang(Cat::Other);
    if other.files > 0 || !other.lines.is_empty() {
        groups.push(vec![row(
            "其他非代码",
            Ink::Plain,
            Some(other.files),
            &other.lines,
            max_total,
            true,
        )]);
    }

    if groups.is_empty() {
        emit(&mut b, &c, vec![(" (无变更)".to_string(), Ink::Faint)]);
    }
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            light(&mut b, &c);
        }
        for cells in g {
            emit(&mut b, &c, cells.clone());
        }
    }

    heavy(&mut b, &c);
    let total = sum_dim(&sum_dim(&rep.code, &rep.test), &rep.other);
    emit(
        &mut b,
        &c,
        row("总计(行)", Ink::Head, Some(n_files), &total, 0, true),
    );
    emit(
        &mut b,
        &c,
        row("文件(个)", Ink::Head, None, &rep.files, 0, false),
    );
    heavy(&mut b, &c);

    if verbose {
        render_files(&mut b, rep, &c);
    }
    b
}

fn render_files(b: &mut String, rep: &Report, c: &Palette) {
    let _ = writeln!(b, " {}", c.paint(Ink::Faint, "变更最大的文件:"));
    for r in rep.top_files().into_iter().take(10) {
        // 目录弱化、文件名常规,更易扫读
        let (dir, base) = match r.path.rsplit_once('/') {
            Some((d, f)) => (format!("{d}/"), f.to_string()),
            None => (String::new(), r.path.clone()),
        };
        let mut parts = Vec::new();
        if r.stat.added > 0 {
            parts.push(c.paint(Ink::Add, &format!("+{}", r.stat.added)));
        }
        if r.stat.deleted > 0 {
            parts.push(c.paint(Ink::Del, &format!("-{}", r.stat.deleted)));
        }
        if r.stat.modified > 0 {
            parts.push(c.paint(Ink::Mod, &format!("~{}", r.stat.modified)));
        }
        if r.stat.binary {
            parts.push(c.paint(Ink::Faint, "二进制"));
        }
        let _ = writeln!(
            b,
            "   {} {}{}  {}",
            c.paint(Ink::Title, &r.status),
            c.paint(Ink::Faint, &dir),
            base,
            parts.join(" ")
        );
    }
}

pub fn color_enabled(mode: &str, is_tty: bool) -> bool {
    match mode {
        "always" => true,
        "never" => false,
        _ => is_tty && std::env::var_os("NO_COLOR").is_none(),
    }
}
