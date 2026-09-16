//! git 调用与按维度聚合。

use std::collections::HashMap;
use std::process::Command;

use crate::classify::{Cat, classify};
use crate::diff::{FileStat, parse_diff};

/// 一个统计维度(行或文件)的 增/删/改 三元组。
#[derive(Debug, Default, Clone, Copy)]
pub struct Dim {
    pub added: usize,
    pub deleted: usize,
    pub modified: usize,
}

impl Dim {
    pub fn is_empty(&self) -> bool {
        self.added == 0 && self.deleted == 0 && self.modified == 0
    }

    fn add(&mut self, s: &FileStat) {
        self.added += s.added;
        self.deleted += s.deleted;
        self.modified += s.modified;
    }
}

/// 单个语言类别的聚合:行数 + 变更文件数。
#[derive(Debug, Default, Clone, Copy)]
pub struct LangStat {
    pub lines: Dim,
    pub files: usize,
}

/// 文件明细表的一行。
pub struct FileRow {
    pub status: String, // A/D/M/R/T/C 首字母
    pub path: String,
    pub cat: Cat,
    pub stat: FileStat,
}

/// 一次统计的完整结果。
pub struct Report {
    pub range_spec: String,
    pub commits: usize, // 0 = 不适用(工作区比较)
    pub code: Dim,      // Go+Python 非测试
    pub test: Dim,
    pub other: Dim,
    pub files: Dim, // added/deleted/modified = 文件个数
    pub by_lang: HashMap<Cat, LangStat>,
    pub file_rows: Vec<FileRow>,
}

impl Report {
    /// 按变更行数降序的文件明细(稳定排序,保持 git 输出的路径序为次序)。
    pub fn top_files(&self) -> Vec<&FileRow> {
        let mut rows: Vec<&FileRow> = self.file_rows.iter().collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.stat.total()));
        rows
    }
}

fn run_git(repo: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(["-C", repo, "-c", "core.quotepath=false"])
        .args(args)
        .output()
        .map_err(|e| format!("无法执行 git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} 失败:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// numstat 单文件条目:新增、删除、是否二进制。
type NumStat = (usize, usize, bool);

/// 解析 `git diff --numstat -z -M` 输出。
/// 普通记录 `A\tD\tpath\0`;重命名记录 `A\tD\t\0old\0new\0`,按新路径入表。
fn parse_numstat(text: &str) -> HashMap<String, NumStat> {
    let mut out = HashMap::new();
    let mut it = text.split('\0');
    while let Some(tok) = it.next() {
        if tok.is_empty() {
            continue;
        }
        let mut parts = tok.splitn(3, '\t');
        let (Some(a), Some(d)) = (parts.next(), parts.next()) else {
            continue;
        };
        let binary = a == "-";
        let entry = (a.parse().unwrap_or(0), d.parse().unwrap_or(0), binary);
        let path = match parts.next() {
            Some("") | None => {
                let _old = it.next();
                let Some(new) = it.next() else { continue };
                new
            }
            Some(p) => p,
        };
        out.insert(path.to_string(), entry);
    }
    out
}

/// 调 git 取 diff 与文件状态,聚合为 Report。
///
/// 总增删以 `--numstat`(git 规范口径)为准;`-U0` 的 hunk 配对只用于
/// 从中拆出"修改"份额——两者的编辑脚本在空行对齐上偶有 ±几行差异,
/// 以 numstat 为准可保证与 `git diff --stat/--numstat` 完全一致。
pub fn collect(repo: &str, range_spec: &str) -> Result<Report, String> {
    let diff_text = run_git(repo, &["diff", "-U0", "-M", "--no-color", range_spec])?;
    let ns_text = run_git(repo, &["diff", "--name-status", "-M", range_spec])?;
    let numstat = parse_numstat(&run_git(
        repo,
        &["diff", "--numstat", "-z", "-M", range_spec],
    )?);

    let mut rep = Report {
        range_spec: range_spec.to_string(),
        commits: 0,
        code: Dim::default(),
        test: Dim::default(),
        other: Dim::default(),
        files: Dim::default(),
        by_lang: HashMap::new(),
        file_rows: Vec::new(),
    };
    if range_spec.contains("..")
        && let Ok(out) = run_git(repo, &["rev-list", "--count", range_spec])
    {
        rep.commits = out.trim().parse().unwrap_or(0);
    }

    let mut stats = parse_diff(&diff_text);
    for line in ns_text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let status = &parts[0][..1]; // R95 -> R
        let path = parts[parts.len() - 1]; // rename 取新路径
        let cat = classify(path);
        let mut stat = stats.remove(path).unwrap_or_default();
        if let Some(&(a_ns, d_ns, bin_ns)) = numstat.get(path) {
            // 修改份额来自 -U0 配对,钳制在 numstat 范围内;纯增/纯删按 numstat 反推
            let m = stat.modified.min(a_ns).min(d_ns);
            stat.added = a_ns - m;
            stat.deleted = d_ns - m;
            stat.modified = m;
            stat.binary |= bin_ns;
        }

        let dim = match cat {
            Cat::GoCode | Cat::PyCode => &mut rep.code,
            Cat::GoTest | Cat::PyTest => &mut rep.test,
            Cat::Other => &mut rep.other,
        };
        dim.add(&stat);
        let ls = rep.by_lang.entry(cat).or_default();
        ls.lines.add(&stat);
        ls.files += 1;

        match status {
            "A" => rep.files.added += 1,
            "D" => rep.files.deleted += 1,
            _ => rep.files.modified += 1, // M / R / T / C
        }
        rep.file_rows.push(FileRow {
            status: status.to_string(),
            path: path.to_string(),
            cat,
            stat,
        });
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_numstat_normal_rename_binary() {
        // 普通 + 重命名(按新路径) + 二进制
        let text = "12\t3\ta/b.go\x004\t5\t\x00old/x.py\x00new/x.py\x00-\t-\tlogo.png\x00";
        let m = parse_numstat(text);
        assert_eq!(m.get("a/b.go"), Some(&(12, 3, false)));
        assert_eq!(m.get("new/x.py"), Some(&(4, 5, false)));
        assert!(!m.contains_key("old/x.py"));
        assert_eq!(m.get("logo.png"), Some(&(0, 0, true)));
    }
}
