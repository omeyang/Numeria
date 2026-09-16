//! 文件路径 → 统计类别。仅保证 Go/Python 的识别。

/// 文件的统计类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cat {
    GoCode,
    PyCode,
    GoTest,
    PyTest,
    Other,
}

impl Cat {
    /// 报告用中文显示名。
    pub fn name(self) -> &'static str {
        match self {
            Cat::GoCode => "Go",
            Cat::PyCode => "Python",
            Cat::GoTest => "Go 测试",
            Cat::PyTest => "Python 测试",
            Cat::Other => "其他",
        }
    }

    /// 按语言明细的固定展示顺序。
    pub const LANG_ORDER: [Cat; 4] = [Cat::GoCode, Cat::PyCode, Cat::GoTest, Cat::PyTest];
}

const GENERATED_SUFFIXES: [&str; 4] = [".pb.go", ".pb.gw.go", "_pb2.py", "_pb2_grpc.py"];

/// 按路径判定统计类别;vendor、node_modules 与生成代码归"其他"。
pub fn classify(path: &str) -> Cat {
    let parts: Vec<&str> = path.split('/').collect();
    let (name, dirs) = parts.split_last().expect("split 至少产生一个元素");

    if dirs.iter().any(|d| *d == "vendor" || *d == "node_modules") {
        return Cat::Other;
    }
    if GENERATED_SUFFIXES.iter().any(|s| name.ends_with(s)) {
        return Cat::Other;
    }

    if name.ends_with("_test.go") {
        Cat::GoTest
    } else if name.ends_with(".go") {
        Cat::GoCode
    } else if name.ends_with(".py") {
        let in_test_dir = dirs.iter().any(|d| *d == "test" || *d == "tests");
        if name.starts_with("test_")
            || name.ends_with("_test.py")
            || *name == "conftest.py"
            || in_test_dir
        {
            Cat::PyTest
        } else {
            Cat::PyCode
        }
    } else {
        Cat::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_by_path() {
        let cases: &[(&str, Cat)] = &[
            // Go
            ("pkg/server/main.go", Cat::GoCode),
            ("a.go", Cat::GoCode),
            ("pkg/server/main_test.go", Cat::GoTest),
            // Python
            ("app/models.py", Cat::PyCode),
            ("app/test_models.py", Cat::PyTest),
            ("app/models_test.py", Cat::PyTest),
            ("conftest.py", Cat::PyTest),
            ("tests/util.py", Cat::PyTest),
            ("a/b/test/helper.py", Cat::PyTest),
            // 生成代码与三方目录归"其他"
            ("api/v1/api.pb.go", Cat::Other),
            ("api/v1/api.pb.gw.go", Cat::Other),
            ("vendor/github.com/x/y.go", Cat::Other),
            ("node_modules/x/y.py", Cat::Other),
            ("rpc/api_pb2.py", Cat::Other),
            ("rpc/api_pb2_grpc.py", Cat::Other),
            // 非代码
            ("README.md", Cat::Other),
            ("deploy/chart.yaml", Cat::Other),
            ("Makefile", Cat::Other),
        ];
        for (path, want) in cases {
            assert_eq!(classify(path), *want, "classify({path:?})");
        }
    }
}
