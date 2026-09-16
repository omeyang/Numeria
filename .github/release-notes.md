Numeria:以古罗马计数女神命名的 git 变更统计工具,`git numeria` 子命令。按 **代码 / 测试代码 / 其他非代码 / 文件** 四个维度输出 新增·删除·修改,只保证 Go 与 Python 的识别;零依赖单二进制。

- `git numeria main feature`:两分支比较(merge-base 三点语法);`git numeria -n 5`:最近 5 个提交。
- `-v` 附加变更最大文件 Top 10;`--html report.html` 生成自包含双主题 HTML 报告。
- 总增删与 `git diff --numstat` 严格一致;"修改" = 同一 hunk 内成对的删除/新增。
- 终端输出为 tokei 式语言分组表格,真彩色 + 比例条,尊重 `NO_COLOR`。

安装:`curl -fsSL https://raw.githubusercontent.com/omeyang/Numeria/main/install.sh | sh`,或下载对应平台的 `git-numeria-<tag>-<target>` 压缩包放入 PATH(含 man 页)。完整文档见 [Wiki](https://github.com/omeyang/Numeria/wiki)。
