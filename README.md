# gongwen-runtime

存放「公文助手」发布流水线所需的离线 TeX runtime 资产。本仓库不提交二进制大文件；
`tectonic/`、`texbundle/`、`fonts/` 由发布方在本地准备，通过 GitHub Release 以
`runtime-<suffix>.zip` 分发。

## Release 契约

应用仓库 `billowsand/gongwen` 的 Release 流水线会按相同 tag 从本仓库下载：

- `runtime-win-x64.zip`
- `runtime-linux-arm64.zip`
- `runtime-linux-amd64.zip`
- `runtime-darwin-arm64.zip`

每个压缩包解压后直接落在应用仓库的 `runtime/` 下，根目录应包含：

```text
tectonic/
  win-x64/tectonic.exe        # Windows x64
  linux-arm64/tectonic        # Linux ARM64
  linux-amd64/tectonic        # Linux AMD64
  darwin-arm64/tectonic       # macOS ARM64
texbundle/gongwen-texlive.ttb
fonts/FangSong.ttf
fonts/KaiTi.ttf
fonts/SimHei.ttf
fonts/SimSun.ttf
fonts/XiaoBiaoSong.ttf
ime/dict.qj                   # 输入法词库（应用内输入法必需）
ime/lm.qj                     # 输入法语言模型（可选，缺了就退到词级候选）
SHA256SUMS.<suffix>.txt
```

`SHA256SUMS.<suffix>.txt` 中的路径与压缩包内布局一致，供
`scripts/package-portable.ps1` 校验。**只列在清单里的文件才会被打进应用包**：
新增数据（如 `ime/`）忘了写进清单，应用装完就会缺文件、而且不报错。

## 构建发布资产

1. 在本地准备好完整 source layout（例如公文助手仓库的 `runtime/` 目录）。
2. 运行：

```powershell
./scripts/build-runtime-release.ps1 -Suffix win-x64 -SourceRuntime D:\gongwen\runtime -OutputDir dist
./scripts/build-runtime-release.ps1 -Suffix linux-arm64 -SourceRuntime D:\gongwen\runtime -OutputDir dist
./scripts/build-runtime-release.ps1 -Suffix linux-amd64 -SourceRuntime D:\gongwen\runtime -OutputDir dist
```

3. 将 `dist/runtime-<suffix>.zip` 上传到本仓库 tag 与应用 tag 一致的 Release。

## 资产来源

- Tectonic 0.17.0 Windows x64 MSVC：官方 release 构建。
- Tectonic 0.17.0 Linux ARM64 musl：官方
  `tectonic-0.17.0-aarch64-unknown-linux-musl.tar.gz` 构建。
- Tectonic 0.17.0 Linux AMD64 musl：官方
  `tectonic-0.17.0-x86_64-unknown-linux-musl.tar.gz` 构建。
- `gongwen-texlive.ttb`：项目专用离线 bundle。
- 字体为部署资产；再分发前需确认授权。
