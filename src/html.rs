//! 自包含 HTML 报告(无外部资源,浅/深双主题)。
//!
//! 配色经 CVD 校验(dataviz validate_palette):
//! - light: 新增 #116329 / 删除 #f05252 / 修改 #0969da — 全项 PASS
//! - dark : 新增 #1c7833 / 删除 #e5665c / 修改 #4493f8 — protan ΔE 6.6(6–8 允许区,
//!   以分段间隙+直标数字+表格视图作二次编码)

use std::fmt::Write as _;

use crate::classify::Cat;
use crate::collect::{Dim, Report};

const CSS: &str = r#"
:root {
  --bg: #fcfcfb; --card: #ffffff; --ink: #1f2328; --ink2: #59636e; --ink3: #818b98;
  --line: #d9dee3; --add: #116329; --del: #f05252; --mod: #0969da;
  --add-soft: #dcefe1; --del-soft: #fde3e1; --mod-soft: #dceafb;
}
@media (prefers-color-scheme: dark) { :root {
  --bg: #1a1a19; --card: #22232a; --ink: #e6e9ec; --ink2: #9aa4af; --ink3: #6e7681;
  --line: #3a3f45; --add: #1c7833; --del: #e5665c; --mod: #4493f8;
  --add-soft: #20301f; --del-soft: #3a2523; --mod-soft: #1e2c42;
} }
:root[data-theme="dark"] {
  --bg: #1a1a19; --card: #22232a; --ink: #e6e9ec; --ink2: #9aa4af; --ink3: #6e7681;
  --line: #3a3f45; --add: #1c7833; --del: #e5665c; --mod: #4493f8;
  --add-soft: #20301f; --del-soft: #3a2523; --mod-soft: #1e2c42;
}
:root[data-theme="light"] {
  --bg: #fcfcfb; --card: #ffffff; --ink: #1f2328; --ink2: #59636e; --ink3: #818b98;
  --line: #d9dee3; --add: #116329; --del: #f05252; --mod: #0969da;
  --add-soft: #dcefe1; --del-soft: #fde3e1; --mod-soft: #dceafb;
}
* { box-sizing: border-box; }
body {
  margin: 0; background: var(--bg); color: var(--ink);
  font: 15px/1.6 -apple-system, "Segoe UI", "PingFang SC", "Noto Sans CJK SC",
        "Microsoft YaHei", Roboto, sans-serif;
}
main { max-width: 880px; margin: 0 auto; padding: 40px 24px 64px; }
h1 { font-size: 21px; margin: 0 0 4px; letter-spacing: .2px; }
h1 code { font: 600 18px/1 ui-monospace, "SFMono-Regular", Consolas, monospace;
  background: var(--card); border: 1px solid var(--line); border-radius: 6px; padding: 2px 8px; }
.meta { color: var(--ink2); font-size: 13px; margin-bottom: 28px; }
.tiles { display: flex; gap: 14px; flex-wrap: wrap; margin-bottom: 30px; }
.tile { flex: 1 1 140px; background: var(--card); border: 1px solid var(--line);
  border-radius: 10px; padding: 14px 18px 12px; }
.tile .num { font-size: 30px; font-weight: 650; font-variant-numeric: tabular-nums; }
.tile .lbl { font-size: 13px; color: var(--ink2); display: flex; align-items: center; gap: 7px; }
.dot { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
.dot.add { background: var(--add); } .dot.del { background: var(--del); } .dot.mod { background: var(--mod); }
section { background: var(--card); border: 1px solid var(--line); border-radius: 12px;
  padding: 22px 24px; margin-bottom: 22px; }
h2 { font-size: 15px; margin: 0 0 16px; color: var(--ink); font-weight: 650; }
.legend { display: flex; gap: 18px; font-size: 12.5px; color: var(--ink2);
  margin: 0 0 18px; align-items: center; }
.legend span { display: inline-flex; align-items: center; gap: 6px; }
.dimrow { display: grid; grid-template-columns: 120px 1fr; gap: 8px 16px;
  align-items: center; padding: 9px 0; }
.dimrow + .dimrow { border-top: 1px solid var(--line); }
.dimrow .name { font-weight: 600; font-size: 14px; }
.barline { display: flex; align-items: center; gap: 12px; min-width: 0; }
.bar { display: flex; height: 12px; border-radius: 4px; overflow: hidden;
  gap: 2px; flex: 0 0 auto; }
.seg { height: 100%; min-width: 4px; }
.seg.add { background: var(--add); } .seg.del { background: var(--del); } .seg.mod { background: var(--mod); }
.seg:hover { filter: brightness(1.18); }
.nums { font-size: 13px; color: var(--ink2); white-space: nowrap;
  font-variant-numeric: tabular-nums; display: flex; gap: 10px; }
.nums b { color: var(--ink); font-weight: 600; }
.chip { display: inline-flex; align-items: center; gap: 6px; border-radius: 6px;
  padding: 2px 10px; font-size: 13px; font-variant-numeric: tabular-nums; }
.chip.add { background: var(--add-soft); } .chip.del { background: var(--del-soft); }
.chip.mod { background: var(--mod-soft); }
.tablewrap { overflow-x: auto; }
table { border-collapse: collapse; width: 100%; font-size: 13.5px; }
th { text-align: left; color: var(--ink2); font-weight: 600; font-size: 12.5px;
  padding: 6px 10px; border-bottom: 1px solid var(--line); white-space: nowrap; }
td { padding: 6px 10px; border-bottom: 1px solid var(--line);
  font-variant-numeric: tabular-nums; }
tr:last-child td { border-bottom: none; }
tr:hover td { background: color-mix(in srgb, var(--line) 26%, transparent); }
td.path { font-family: ui-monospace, "SFMono-Regular", Consolas, monospace; font-size: 12.5px;
  word-break: break-all; }
td.n { text-align: right; white-space: nowrap; }
.st { font: 600 11px/1 ui-monospace, monospace; border: 1px solid var(--line);
  border-radius: 4px; padding: 2px 5px; color: var(--ink2); }
.cat { color: var(--ink3); font-size: 12px; white-space: nowrap; }
.minibar { display: flex; height: 6px; border-radius: 3px; overflow: hidden; gap: 2px;
  min-width: 60px; max-width: 140px; }
footer { color: var(--ink3); font-size: 12px; margin-top: 26px; }
"#;

const LEGEND: &str = r#"<div class="legend"><span><i class="dot add"></i>新增</span><span><i class="dot del"></i>删除</span><span><i class="dot mod"></i>修改</span></div>"#;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn seg_vals(d: &Dim) -> [(&'static str, &'static str, usize); 3] {
    [
        ("add", "新增", d.added),
        ("del", "删除", d.deleted),
        ("mod", "修改", d.modified),
    ]
}

/// 比例条 + 直标数字;宽度按全局 scale(px/单位),零段省略。
fn bar(d: &Dim, scale: f64, unit: &str) -> String {
    let mut segs = String::new();
    let mut nums = String::new();
    for (class, label, val) in seg_vals(d) {
        if val == 0 {
            continue;
        }
        let w = ((val as f64 * scale).round() as usize).max(4);
        let _ = write!(
            segs,
            r#"<i class="seg {class}" style="width:{w}px" title="{label} {val} {unit}"></i>"#
        );
        let _ = write!(nums, "<span>{label} <b>{val}</b> {unit}</span>");
    }
    format!(
        r#"<div class="barline"><div class="bar">{segs}</div><div class="nums">{nums}</div></div>"#
    )
}

fn chips(d: &Dim, unit: &str) -> String {
    let mut out = String::new();
    for (class, label, val) in seg_vals(d) {
        if val == 0 {
            continue;
        }
        let _ = write!(
            out,
            r#"<span class="chip {class}"><i class="dot {class}"></i>{label} <b>&nbsp;{val}</b>&nbsp;{unit}</span>"#
        );
    }
    format!(r#"<div class="barline" style="gap:8px;flex-wrap:wrap">{out}</div>"#)
}

pub fn render(rep: &Report) -> String {
    let line_dims: [(&str, &Dim); 3] = [
        ("代码", &rep.code),
        ("测试代码", &rep.test),
        ("其他非代码", &rep.other),
    ];
    let max_total = line_dims
        .iter()
        .map(|(_, d)| d.added + d.deleted + d.modified)
        .max()
        .unwrap_or(0)
        .max(1);
    let scale = 320.0 / max_total as f64; // 最长条 320px,跨维度可比

    let total_add: usize = line_dims.iter().map(|(_, d)| d.added).sum();
    let total_del: usize = line_dims.iter().map(|(_, d)| d.deleted).sum();
    let total_mod: usize = line_dims.iter().map(|(_, d)| d.modified).sum();

    let mut meta = Vec::new();
    if rep.commits > 0 {
        meta.push(format!("{} 个提交", rep.commits));
    }
    let n_files = rep.files.added + rep.files.deleted + rep.files.modified;
    meta.push(format!("{n_files} 个文件变更"));

    let mut tiles = String::new();
    for (class, label, val) in [
        ("add", "总新增", total_add),
        ("del", "总删除", total_del),
        ("mod", "总修改", total_mod),
    ] {
        if val == 0 {
            continue;
        }
        let _ = write!(
            tiles,
            r#"<div class="tile"><div class="num">{val}</div><div class="lbl"><i class="dot {class}"></i>{label}(行)</div></div>"#
        );
    }

    let mut dim_rows = String::new();
    for (name, d) in &line_dims {
        if d.is_empty() {
            continue;
        }
        let _ = write!(
            dim_rows,
            r#"<div class="dimrow"><div class="name">{name}</div>{}</div>"#,
            bar(d, scale, "行")
        );
    }
    if !rep.files.is_empty() {
        let _ = write!(
            dim_rows,
            r#"<div class="dimrow"><div class="name">文件</div>{}</div>"#,
            chips(&rep.files, "个")
        );
    }

    let mut lang_rows = String::new();
    for cat in Cat::LANG_ORDER {
        let Some(d) = rep
            .by_lang
            .get(&cat)
            .map(|l| l.lines)
            .filter(|d| !d.is_empty())
        else {
            continue;
        };
        let _ = write!(
            lang_rows,
            r#"<div class="dimrow"><div class="name">{}</div>{}</div>"#,
            escape(cat.name()),
            bar(&d, scale, "行")
        );
    }
    let lang_section = if lang_rows.is_empty() {
        String::new()
    } else {
        format!("<section><h2>按语言</h2>{LEGEND}{lang_rows}</section>")
    };

    let top = rep.top_files();
    let f_max = top.iter().map(|r| r.stat.total()).max().unwrap_or(0).max(1);
    let f_scale = 140.0 / f_max as f64;
    let mut rows = String::new();
    for r in &top {
        let mut cells = String::new();
        let d = Dim {
            added: r.stat.added,
            deleted: r.stat.deleted,
            modified: r.stat.modified,
        };
        for (class, _, val) in seg_vals(&d) {
            if val == 0 {
                continue;
            }
            let w = ((val as f64 * f_scale).round() as usize).max(3);
            let _ = write!(cells, r#"<i class="seg {class}" style="width:{w}px"></i>"#);
        }
        let mini = if !cells.is_empty() {
            format!(r#"<div class="minibar">{cells}</div>"#)
        } else if r.stat.binary {
            r#"<span class="cat">二进制</span>"#.to_string()
        } else {
            String::new()
        };
        let mut nums = Vec::new();
        if r.stat.added > 0 {
            nums.push(format!("+{}", r.stat.added));
        }
        if r.stat.deleted > 0 {
            nums.push(format!("−{}", r.stat.deleted));
        }
        if r.stat.modified > 0 {
            nums.push(format!("~{}", r.stat.modified));
        }
        let _ = write!(
            rows,
            r#"<tr><td><span class="st">{}</span></td><td class="path">{}</td><td class="cat">{}</td><td class="n">{}</td><td>{}</td></tr>"#,
            escape(&r.status),
            escape(&r.path),
            escape(r.cat.name()),
            nums.join(" "),
            mini
        );
    }

    let range = escape(&rep.range_spec);
    format!(
        r#"<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Numeria · {range}</title>
<style>{CSS}</style>
<main>
<h1>变更统计 <code>{range}</code></h1>
<div class="meta">{meta}</div>
<div class="tiles">{tiles}</div>
<section><h2>维度总览</h2>{LEGEND}{dim_rows}</section>
{lang_section}
<section><h2>文件明细</h2><div class="tablewrap">
<table><thead><tr><th></th><th>路径</th><th>类别</th><th>行数</th><th></th></tr></thead>
<tbody>{rows}</tbody></table>
</div></section>
<footer>Numeria · 修改行 = 同一 hunk 内成对的删除/新增(min 配对),余量计为纯增/纯删</footer>
</main>"#,
        meta = meta.join(" · "),
    )
}
