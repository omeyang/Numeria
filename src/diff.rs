//! unified diff 解析与范围语义。
//!
//! "修改行"的定义:git 原生只有增/删,同一 hunk 内 min(删除, 新增) 计为修改,
//! 余量计纯增/纯删(配合 `git diff -U0` 精确配对)。

use std::collections::HashMap;

/// 单个文件的行级统计。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub added: usize,
    pub deleted: usize,
    pub modified: usize,
    pub binary: bool,
}

impl FileStat {
    pub fn total(&self) -> usize {
        self.added + self.deleted + self.modified
    }
}

/// 解析 `git diff -U0 -M` 的输出,返回 path -> FileStat。
pub fn parse_diff(text: &str) -> HashMap<String, FileStat> {
    let mut stats = HashMap::new();
    // 按文件块切分:每块以 "diff --git " 开头
    let mut cur: Option<Block> = None;
    for line in text.lines() {
        if line.starts_with("diff --git ") {
            if let Some(b) = cur.take() {
                b.commit(&mut stats);
            }
            cur = Some(Block::default());
            continue;
        }
        let Some(b) = cur.as_mut() else { continue };
        b.feed(line);
    }
    if let Some(b) = cur.take() {
        b.commit(&mut stats);
    }
    stats
}

/// 一个文件的 diff 块,边扫边攒。
#[derive(Default)]
struct Block {
    old_path: Option<String>,
    new_path: Option<String>,
    stat: FileStat,
}

impl Block {
    fn feed(&mut self, line: &str) {
        if let Some(p) = line.strip_prefix("--- ") {
            if p != "/dev/null" {
                self.old_path = Some(unquote_path(p.strip_prefix("a/").unwrap_or(p)));
            }
        } else if let Some(p) = line.strip_prefix("+++ ") {
            if p != "/dev/null" {
                self.new_path = Some(unquote_path(p.strip_prefix("b/").unwrap_or(p)));
            }
        } else if let Some(p) = line.strip_prefix("rename to ") {
            self.new_path = Some(unquote_path(p));
        } else if line.starts_with("Binary files ") {
            self.stat.binary = true;
            if self.new_path.is_none()
                && let Some(p) = binary_path(line)
            {
                self.new_path = Some(p);
            }
        } else if let Some((old_n, new_n)) = parse_hunk_header(line) {
            let modified = old_n.min(new_n);
            self.stat.modified += modified;
            self.stat.added += new_n - modified;
            self.stat.deleted += old_n - modified;
        }
    }

    fn commit(self, stats: &mut HashMap<String, FileStat>) {
        // 删除的文件用旧路径;重命名/新增用新路径
        if let Some(path) = self.new_path.or(self.old_path) {
            stats.insert(path, self.stat);
        }
    }
}

/// 解析 hunk 头 `@@ -a[,b] +c[,d] @@`,返回 (旧行数, 新行数);行数缺省为 1。
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_spec, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix('+')?;
    let (new_spec, rest) = rest.split_once(' ')?;
    if !rest.starts_with("@@") {
        return None;
    }
    Some((spec_count(old_spec)?, spec_count(new_spec)?))
}

/// `start` 或 `start,count` → count(缺省 1)。
fn spec_count(spec: &str) -> Option<usize> {
    match spec.split_once(',') {
        Some((start, count)) => {
            start.parse::<usize>().ok()?;
            count.parse().ok()
        }
        None => {
            spec.parse::<usize>().ok()?;
            Some(1)
        }
    }
}

/// 从 `Binary files a/x and b/y differ` 提取路径(优先新侧)。
fn binary_path(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("Binary files ")?
        .strip_suffix(" differ")?;
    let (a, b) = rest.rsplit_once(" and ")?;
    let pick = if b != "/dev/null" { b } else { a };
    let pick = pick
        .strip_prefix("b/")
        .or_else(|| pick.strip_prefix("a/"))
        .unwrap_or(pick);
    Some(unquote_path(pick))
}

/// 处理 git 对特殊字符路径的 C 风格引号包裹(core.quotepath=false 后仍可能出现)。
fn unquote_path(p: &str) -> String {
    let Some(inner) = p.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return p.to_string();
    };
    let mut bytes = Vec::with_capacity(inner.len());
    let mut it = inner.bytes().peekable();
    while let Some(c) = it.next() {
        if c != b'\\' {
            bytes.push(c);
            continue;
        }
        match it.next() {
            Some(b't') => bytes.push(b'\t'),
            Some(b'n') => bytes.push(b'\n'),
            Some(d @ b'0'..=b'7') => {
                let mut v = (d - b'0') as u32;
                for _ in 0..2 {
                    match it.peek() {
                        Some(d @ b'0'..=b'7') => {
                            v = v * 8 + (*d - b'0') as u32;
                            it.next();
                        }
                        _ => break,
                    }
                }
                bytes.push(v as u8);
            }
            Some(other) => bytes.push(other), // \\ \" 等
            None => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// 把命令行位置参数翻译为 git 范围:
/// 双参 → `A...B`(merge-base);含 ".." 原样透传;单 rev → `rev..HEAD`;无参 → 工作区 vs HEAD。
pub fn resolve_range(refs: &[String]) -> String {
    match refs {
        [] => "HEAD".to_string(),
        [a, b] => format!("{a}...{b}"),
        [r, ..] if r.contains("..") => r.clone(),
        [r, ..] => format!("{r}..HEAD"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(diff: &str, path: &str) -> FileStat {
        let stats = parse_diff(diff);
        stats
            .get(path)
            .unwrap_or_else(|| panic!("path {path:?} not found, keys: {:?}", stats.keys()))
            .clone()
    }

    #[test]
    fn pure_addition() {
        let s = stat(
            "diff --git a/a.go b/a.go\nindex 000..111 100644\n--- a/a.go\n+++ b/a.go\n@@ -0,0 +5,3 @@\n+x\n+y\n+z\n",
            "a.go",
        );
        assert_eq!((s.added, s.deleted, s.modified), (3, 0, 0));
    }

    #[test]
    fn paired_hunk_counts_as_modified() {
        // 一个 hunk 删 2 加 3 → 修改 2,新增 1
        let s = stat(
            "diff --git a/a.py b/a.py\nindex 000..111 100644\n--- a/a.py\n+++ b/a.py\n@@ -10,2 +10,3 @@\n-old1\n-old2\n+new1\n+new2\n+new3\n",
            "a.py",
        );
        assert_eq!((s.added, s.deleted, s.modified), (1, 0, 2));
    }

    #[test]
    fn multiple_hunks_and_default_count() {
        // 头部无 ",n" 时默认 1 行
        let s = stat(
            "diff --git a/a.go b/a.go\nindex 000..111 100644\n--- a/a.go\n+++ b/a.go\n@@ -3 +3 @@\n-old\n+new\n@@ -10,2 +10,0 @@\n-gone1\n-gone2\n",
            "a.go",
        );
        assert_eq!((s.added, s.deleted, s.modified), (0, 2, 1));
    }

    #[test]
    fn deleted_file_uses_old_path() {
        let s = stat(
            "diff --git a/dead.py b/dead.py\ndeleted file mode 100644\nindex 000..111\n--- a/dead.py\n+++ /dev/null\n@@ -1,4 +0,0 @@\n-a\n-b\n-c\n-d\n",
            "dead.py",
        );
        assert_eq!((s.added, s.deleted, s.modified), (0, 4, 0));
    }

    #[test]
    fn binary_file_has_no_line_stats() {
        let s = stat(
            "diff --git a/logo.png b/logo.png\nindex 000..111 100644\nBinary files a/logo.png and b/logo.png differ\n",
            "logo.png",
        );
        assert!(s.binary);
        assert_eq!((s.added, s.deleted, s.modified), (0, 0, 0));
    }

    #[test]
    fn rename_uses_new_path() {
        let s = stat(
            "diff --git a/old/name.go b/new/name.go\nsimilarity index 90%\nrename from old/name.go\nrename to new/name.go\n--- a/old/name.go\n+++ b/new/name.go\n@@ -5,1 +5,1 @@\n-x\n+y\n",
            "new/name.go",
        );
        assert_eq!((s.added, s.deleted, s.modified), (0, 0, 1));
    }

    #[test]
    fn resolve_range_semantics() {
        let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(resolve_range(&s(&["main", "feature"])), "main...feature");
        assert_eq!(resolve_range(&s(&["a..b"])), "a..b");
        assert_eq!(resolve_range(&s(&["a...b"])), "a...b");
        assert_eq!(resolve_range(&s(&["HEAD~5"])), "HEAD~5..HEAD");
        assert_eq!(resolve_range(&[]), "HEAD");
    }

    #[test]
    fn unquote_octal_utf8_path() {
        // git 引号包裹的中文路径(UTF-8 octal 转义)
        assert_eq!(unquote_path(r#""doc/\344\270\255.md""#), "doc/中.md");
        assert_eq!(unquote_path("plain/path.go"), "plain/path.go");
    }
}
