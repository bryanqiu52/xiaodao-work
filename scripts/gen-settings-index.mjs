#!/usr/bin/env node
// 生成设置项搜索索引。
//
// **为什么必须在构建期生成**：设置面板是**按大类懒加载**的 —— 没点开的那几个大类
// 根本不在 DOM 里。运行时遍历 DOM 只能搜到当前这一页，搜不到别的页。
// 所以把面板模板里的标题抽出来，存成一份静态索引，搜索时查它。
//
// 约定的模板结构（每个面板文件里，一个分区一块）：
//   <section id="sv-sec-<分区id>" class="sv-sec">
//     <h3 class="sv-sec-title">分区名</h3>
//     <div class="setting-row"> … <span class="setting-name">设置项标题</span> … </div>
//   </section>
//
// 用法：
//   node scripts/gen-settings-index.mjs          生成/刷新索引
//   node scripts/gen-settings-index.mjs --check  只校验是否一致（不一致退码 1），不写文件
//
// `npm run build` 前会自动跑一次（见 package.json 的 prebuild）。

import { readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const panelsDir = join(root, 'src', 'components', 'settings')
const outFile = join(root, 'src', 'components', 'settingsIndex.generated.ts')

/** 扫哪些面板：显式列出来，避免把外壳组件也扫进去 */
const PANELS = [
  'GeneralPanel.vue',
  'AppearancePanel.vue',
  'TodoPanel.vue',
  'ToolsPanel.vue',
  'OpenPanel.vue',
  'DataPanel.vue',
  'AboutPanel.vue',
]

const SECTION_RE = /<section\s+id="sv-sec-([\w-]+)"[^>]*>([\s\S]*?)<\/section>/g
const TITLE_RE = /<span class="setting-name">([^<>{}]+)<\/span>/g

function collect() {
  const entries = []
  // 巡检一遍目录，确认约定的面板文件都在（改名了要第一时间发现，而不是静默少搜几项）
  const present = new Set(readdirSync(panelsDir))
  const missing = PANELS.filter((p) => !present.has(p))
  if (missing.length > 0) {
    console.error(`[设置索引] 找不到这些面板文件：${missing.join(', ')}`)
    process.exit(1)
  }

  for (const name of PANELS) {
    const text = readFileSync(join(panelsDir, name), 'utf8')
    SECTION_RE.lastIndex = 0
    let sec
    while ((sec = SECTION_RE.exec(text)) !== null) {
      const section = sec[1]
      const body = sec[2]
      TITLE_RE.lastIndex = 0
      let t
      while ((t = TITLE_RE.exec(body)) !== null) {
        const title = t[1].trim()
        if (title) entries.push({ section, title })
      }
    }
  }
  return entries
}

function render(entries) {
  const lines = entries.map((e) => `  { section: '${e.section}', title: ${JSON.stringify(e.title)} },`)
  return `// 此文件由 scripts/gen-settings-index.mjs 生成，请勿手改。
//
// 设置面板按大类懒加载，没点开的大类不在 DOM 里 —— 搜索只能查这份静态索引。
// 改完面板模板后跑 \`npm run gen:settings-index\`，或直接 \`npm run build\`（会自动跑）。

/** 一条设置项索引：它在哪个分区、标题是什么（分区到大类由 SettingsView 查表得到） */
export interface SettingsIndexEntry {
  section: string
  title: string
}

export const SETTINGS_INDEX: SettingsIndexEntry[] = [
${lines.join('\n')}
]
`
}

const entries = collect()
const next = render(entries)
const norm = (s) => s.replace(/\r\n/g, '\n').trim()

if (process.argv.includes('--check')) {
  let current = ''
  try {
    current = readFileSync(outFile, 'utf8')
  } catch {
    // 文件不存在 = 不一致
  }
  if (norm(current) !== norm(next)) {
    console.error('[设置索引] 与模板不一致 —— 跑 `npm run gen:settings-index` 重新生成')
    process.exit(1)
  }
  console.log(`[设置索引] 已是最新（${entries.length} 项）`)
} else {
  writeFileSync(outFile, next, 'utf8')
  console.log(`[设置索引] 已生成 ${entries.length} 项`)
}
