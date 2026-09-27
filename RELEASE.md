# 发版流程

> 这份是**给自己看的操作手册**。README 给外人看，这份给"要发版的人"看。
> 一次配好之后，日常发版只走「日常发版」那一节。

## 这条链子长什么样

```
改代码 → 本地跑通 → 改版本号 → 写更新日志 → 生成 release 说明
   → commit + 打 tag → push
   → GitHub Actions 自动：装环境 → 构建 → 出安装包 + 签名
   → 建 Release（草稿）→ 人确认后发布
   → 用户软件每天查一次 Releases → 比当前版本新 → 弹窗提示 → 一键更新
```

## 一次性准备（只做一次）

### 1. 仓库

- 仓库：`https://github.com/bryanqiu52/xiaodao-work`（**公开**）
- 许可证：MIT
- 注意：用户名以后要是改成品牌名，GitHub 会自动跳转旧地址，
  **已经装出去的老版本照样能查到更新**；但代码里那个常量要跟着改（见下）

### 2. 更新签名密钥（**唯一的不可逆步骤**）

自动更新要能验证"这个包确实是本项目发的、没被人换过"，所以要有签名密钥。

```powershell
# 在项目根目录跑。它会问你要不要给私钥设密码 —— **建议设**，然后记进密码管理器
npm run tauri signer generate -- -w "$env:USERPROFILE\.tauri\xiaodao-work.key"
```

它会产出两个东西：

| | 是什么 | 放哪 |
|:--|:--|:--|
| `xiaodao-work.key` | **私钥**，用来签名，绝不能泄露 | ① GitHub Secrets ② 你的密码管理器 ③ 「数字世界」的证照资料区 |
| 打印出来的那串 | **公钥**，写进 `tauri.conf.json` 的 `plugins.updater.pubkey` | 进仓库，公开的 |

**⚠️ 私钥丢了会怎样**：以后发出去的包**永远无法被自动更新**，
已经装了的老用户只能手动重装一次才能回到更新链路上。密钥能重新生成，但新旧对不上。
所以存三份，别只放本地。

`*.key` 已经在 `.gitignore` 里 —— **任何情况下都别把它提交上去**。

**本地出包也要带着它。** 打开 `createUpdaterArtifacts` 之后，`tauri build` 会要求签名密钥 ——
没设就**直接失败**（这是故意的：签不出名的包一旦发出去，用户端校验不过、装不上）。
所以本地每次出包前先设上：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\xiaodao-work.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run tauri:build
```

（这两个变量**只在当前这个终端窗口有效**，重开终端要再设一遍。
密码留空是因为生成密钥时没设密码。）

### 3. GitHub Secrets

仓库 → Settings → Secrets and variables → Actions，加两条：

| Name | Value |
|:--|:--|
| `TAURI_SIGNING_PRIVATE_KEY` | 私钥文件的**全部内容**（把 `.key` 文件内容整段复制） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 上面设的那个密码（没设密码就留空字符串） |

CI 靠这两条给安装包签名 —— 少了它们，构建会失败（这是故意的：**签不出名的包别发**）。

## 日常发版

### 第 1 步：改版本号（**四处，漏一处就出鬼**）

| 文件 | 字段 |
|:--|:--|
| `package.json` | `"version"` |
| `src-tauri/tauri.conf.json` | `"version"` |
| `src-tauri/Cargo.toml` | `version` |
| `src/components/settings/AboutPanel.vue` | 顶部 `const VERSION` |

版本号规则（语义化，别跳）：

- **补丁位** `1.1.1 → 1.1.2`：修 bug、改文案、调样式
- **次版本** `1.1.2 → 1.2.0`：加功能
- **主版本** `1.2.0 → 2.0.0`：改数据结构、老用户要手动做什么才能升级

### 第 2 步：写更新日志

改 `AboutPanel.vue` 里 `CHANGELOG` 数组的**最前面**加一条：版本号、日期、`items`。

**只写用户能感知到的变化。** 重构、加注释、改构建脚本不进这里 ——
用户翻更新日志是想知道"我升级能得到什么"，写内部细节就是噪音。

### 第 3 步：生成 release 说明

新建 `docs/releases/v<版本号>.md`（**文件名必须和 tag 一致**，比如 tag 是 `v1.1.2` 就写 `v1.1.2.md`）。

内容 = 给用户看的版本说明，跟更新日志同源但更完整些：
开头一句话概括这一版的重点，然后分组列变化，最后写"升级要注意什么"（如果有）。

CI 会拿这个文件当 GitHub Release 的正文 —— **不用在网页上手打**。

### 第 4 步：提交、打 tag、推

```powershell
cd "E:\软件开发\04-程序开发\xiaodao_work"
git add -A
git commit -m "release: v1.1.2"
git tag v1.1.2
git push --follow-tags
```

推上去之后 GitHub Actions 自动开跑（约 5–10 分钟）。

### 第 5 步：确认并发布

去仓库的 **Actions** 看构建有没有绿；绿了之后到 **Releases** 会看到一个**草稿**：

- 检查安装包在不在、说明文字对不对
- 顺手把安装包在一台干净环境（或换个目录）装一次，确认能打开
- 没问题点 **Publish release**

**草稿状态对用户不可见** —— 更新检查读的是"已发布的最新版"，所以草稿期间没人会被更新到。
这是故意的：宁可晚一步，不要发出去一个装不上的包。

## 出事了怎么办

| 情况 | 怎么办 |
|:--|:--|
| 构建红了 | 看 Actions 日志。90% 是 Secrets 没配、或 `docs/releases/vX.md` 文件名和 tag 对不上 |
| 发出去才发现有问题 | **不要删 Release**（已经有人更新了）。改代码、升版本号、发下一版，在说明里写清修了什么 |
| 只是说明写错了 | Releases 里点编辑改正文，不影响更新链路 |
| 私钥丢了 | 生成新的，把新公钥写进 `tauri.conf.json` 发一版。**老用户要手动重装一次**才能回到自动更新 —— 这就是为什么私钥要存三份 |

## 发布前自检（每次过一遍）

- [ ] 版本号**四处**都改了，且一致
- [ ] `CHANGELOG` 加了条目，`docs/releases/vX.Y.Z.md` 建了，**文件名和 tag 一致**
- [ ] `git status` 里没有意外文件（尤其别把 `.key`、`data/`、`待办.json` 加进来）
- [ ] 本地 `npm run tauri:dev` 起来跑一遍要改的那几个地方
- [ ] 本地出包前设好了 `TAURI_SIGNING_PRIVATE_KEY`（不设会构建失败，见上面那节）
- [ ] 本地 `npm run tauri:build` 能出包，并且**按 AGENTS.md 那套验包流程跑一次**（启动日志里要有 `启动自检 version=<新版本号>`）

## 代码里那两个常量（改名时只改这处）

- GitHub 地址：`src/core/constants.ts` 的 `GITHUB_OWNER` / `GITHUB_REPO`
- 更新检查读的就是它拼出来的 `/releases/latest` 接口

用户名或仓库名改了，只改这里 + `README.md` 里的链接。
