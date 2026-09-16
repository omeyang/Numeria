//! 端到端测试:构造真实 git 仓库,数字与手工核算值比对。

use std::fs;
use std::path::Path;
use std::process::Command;

fn git(repo: &Path, args: &[&str]) {
    let st = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .status()
        .expect("git 可执行");
    assert!(st.success(), "git {args:?} 失败");
}

fn write(repo: &Path, rel: &str, content: &str) {
    let p = repo.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

#[test]
fn demo_repo_dimensions() {
    let dir = std::env::temp_dir().join(format!("numeria-e2e-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let repo = dir.as_path();

    git(repo, &["init", "-q", "-b", "main"]);
    write(
        repo,
        "pkg/server/main.go",
        "package server\n\nfunc A() int {\n\treturn 1\n}\n\nfunc B() int {\n\treturn 2\n}\n",
    );
    write(
        repo,
        "pkg/server/main_test.go",
        "package server\n\nfunc TestA(t *T) {\n\tA()\n}\n",
    );
    write(
        repo,
        "app.py",
        "def handler(x):\n    return x + 1\n\ndef helper(y):\n    return y * 2\n",
    );
    write(
        repo,
        "tests/test_app.py",
        "from app import handler\n\ndef test_handler():\n    assert handler(1) == 2\n",
    );
    write(repo, "docs/README.md", "# Demo\n\nhello\n");
    write(
        repo,
        "api/v1/api.pb.go",
        "generated line 1\ngenerated line 2\n",
    );
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-qm", "init"]);

    git(repo, &["switch", "-qc", "feature"]);
    // main.go: 修改 1 行 + 新增 4 行(新函数)
    write(
        repo,
        "pkg/server/main.go",
        "package server\n\nfunc A() int {\n\treturn 10\n}\n\nfunc B() int {\n\treturn 2\n}\n\nfunc D() int {\n\treturn 4\n}\n",
    );
    // 新文件 3 行
    write(repo, "pkg/server/util.go", "package server\n\nvar X = 1\n");
    // app.py: 删除 1 行
    write(
        repo,
        "app.py",
        "def handler(x):\n    return x + 1\n\ndef helper(y):\n",
    );
    // 测试新增 3 行
    write(
        repo,
        "tests/test_app.py",
        "from app import handler\n\ndef test_handler():\n    assert handler(1) == 2\n\ndef test_two():\n    assert True\n",
    );
    git(repo, &["rm", "-q", "docs/README.md"]);
    // pb.go 修改 1 行
    write(
        repo,
        "api/v1/api.pb.go",
        "generated CHANGED 1\ngenerated line 2\n",
    );
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-qm", "change1"]);
    write(repo, "conf.yaml", "config: yes\n");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-qm", "change2"]);

    let out = Command::new(env!("CARGO_BIN_EXE_git-numeria"))
        .args([
            "-C",
            repo.to_str().unwrap(),
            "--color",
            "never",
            "main",
            "feature",
        ])
        .output()
        .expect("运行 git-numeria");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout).into_owned();

    // 手工核算的期望值(与 Python/Go 版对拍一致);语言分组 + |- 测试子行
    for want in [
        "Numeria · main...feature",
        "2 个提交 · 7 个文件变更",
        " Go                 2        +7                  ~1  ██████████████████",
        " Python             1                  -1            ██░░░░░░░░░░░░░░░░",
        " |- 测试            1        +3                      ███████░░░░░░░░░░░",
        " (合计)             2        +3        -1",
        " 其他非代码         3        +1        -3        ~1  ███████████░░░░░░░",
        " 总计(行)           7       +11        -4        ~2",
        " 文件(个)                     2         1         4",
    ] {
        assert!(text.contains(want), "缺少 {want:?},实际输出:\n{text}");
    }
    // 0 值留空:|- 测试 行只有 +3 与 █ 纹理,无删除/修改痕迹
    let test_row = text
        .lines()
        .find(|l| l.contains("|- 测试"))
        .expect("有 |- 测试 行");
    let has_deleted_num = test_row
        .as_bytes()
        .windows(2)
        .any(|w| w[0] == b'-' && w[1].is_ascii_digit());
    assert!(
        !has_deleted_num
            && !test_row.contains('~')
            && !test_row.contains('▒')
            && !test_row.contains('▓'),
        "0 值未留空:\n{text}"
    );

    let _ = fs::remove_dir_all(&dir);
}
