//! git-numeria — 按维度统计 git 变更:代码 / 测试代码 / 其他非代码 / 文件,
//! 各输出 新增·删除·修改(为 0 的项省略)。仅保证 Go 与 Python 的识别。

mod classify;
mod collect;
mod diff;
mod html;
mod term;

use std::io::IsTerminal;
use std::process::ExitCode;

const USAGE: &str = "\
git-numeria — 按维度统计 git 变更(代码/测试/其他非代码/文件)

用法:
  git numeria [<选项>] [<范围>]

范围:
  main feature                    两分支比较(merge-base 三点语法 main...feature)
  HEAD~5                          最近 5 个提交(等价 -n 5)
  v1.0..v2.0                      显式范围原样透传(两点/三点均可)
  (无参数)                        工作区未提交改动 vs HEAD

选项:
  -n <N>          统计最近 N 个提交(HEAD~N..HEAD)
  -C <path>       git 仓库路径(默认当前目录)
  -v              附加变更最大文件 Top 10 明细
  --html <file>   额外输出自包含 HTML 报告(浅/深双主题)
  --color <mode>  auto|always|never(默认 auto,仅终端着色)
  -V, --version   显示版本号
  -h, --help      显示本帮助

维度:
  代码            *.go / *.py(排除测试与生成代码)
  测试代码        *_test.go;test_*.py、*_test.py、conftest.py、tests?/ 下的 .py
  其他非代码      其余全部,含 vendor/、node_modules/、*.pb.go、*_pb2*.py
  文件            新增/删除/修改的文件个数

说明:
  总增删与 git diff --numstat 严格一致;\"修改\" = 同一 hunk 内成对的
  删除/新增(min 配对),余量计为纯增/纯删。为 0 的格留空。
  flag 可写在范围之后,-flag / --flag / -flag=value 均可。

文档: https://github.com/omeyang/Numeria/wiki";

struct Args {
    refs: Vec<String>,
    n: usize,
    repo: String,
    verbose: bool,
    html: Option<String>,
    color: String,
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut args = Args {
        refs: Vec::new(),
        n: 0,
        repo: ".".to_string(),
        verbose: false,
        html: None,
        color: "auto".to_string(),
    };
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        // 支持 -flag、--flag、-flag=value 三种形式;flag 可在位置参数之后
        let (name, inline_val) = match a.strip_prefix("--").or_else(|| a.strip_prefix('-')) {
            Some(n) if !n.is_empty() => match n.split_once('=') {
                Some((k, v)) => (k, Some(v.to_string())),
                None => (n, None),
            },
            _ => {
                args.refs.push(a.clone());
                continue;
            }
        };
        let mut value = |flag: &str| -> Result<String, String> {
            inline_val
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or_else(|| format!("{flag} 需要一个参数"))
        };
        match name {
            "n" => {
                args.n = value("-n")?
                    .parse()
                    .map_err(|_| "-n 需要整数".to_string())?
            }
            "C" | "repo" => args.repo = value("-C")?,
            "v" | "verbose" => args.verbose = true,
            "html" => args.html = Some(value("--html")?),
            "color" => args.color = value("--color")?,
            "h" | "help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "V" | "version" => {
                println!("git-numeria {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other => return Err(format!("未知选项 -{other}(-h 查看用法)")),
        }
    }
    Ok(args)
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse_args(&argv)?;

    let range_spec = if args.n > 0 {
        if !args.refs.is_empty() {
            return Err("-n 与位置参数不能同时使用".to_string());
        }
        format!("HEAD~{}..HEAD", args.n)
    } else {
        diff::resolve_range(&args.refs)
    };

    let rep = collect::collect(&args.repo, &range_spec)?;

    let color = term::color_enabled(&args.color, std::io::stdout().is_terminal());
    print!("{}", term::render(&rep, color, args.verbose));

    if let Some(path) = &args.html {
        std::fs::write(path, html::render(&rep)).map_err(|e| format!("写入 HTML 报告失败: {e}"))?;
        println!("  HTML 报告已生成: {path}\n");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("git-numeria: {e}");
            ExitCode::FAILURE
        }
    }
}
