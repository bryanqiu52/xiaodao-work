<script setup lang="ts">
// 新建 / 编辑的完整字段区（标题和正文由外层持有，这里只管"其余字段"）。
//
// 提交后**保留**领域 / 权重 / 归属 / 状态（只清标题、正文、链接、期限）——
// 连着记一批同类待办时不用每次重选。
import { ref } from 'vue'
import { X } from 'lucide-vue-next'
import {
  PRIORITY_LABELS,
  PRIORITY_OPTIONS,
  STATUS_ACTION_LABELS,
  STATUS_OPTIONS,
  OWNER_LABELS,
  OWNER_OPTIONS,
  artifactIcon,
  toLibraryRelative,
  withDirSlash,
} from '../core/constants'
import { useDomains } from '../composables/useDomains'

const { options: domainOptions, label: domainName } = useDomains()
import { nowLocalMinute, type DraftFields } from '../core/draft'
import { baseName } from '../core/brief'
import { shortId } from '../core/view'
import { tauriApi } from '../api/tauri'
import { configStore } from '../stores/config'
import TaskPicker from './TaskPicker.vue'

/**
 * `withTitle`：连标题和正文也一起放进这个编辑器里。
 * 底部升起的那张完整表单要它（用户在表单里就能改完所有字段，不用回头找输入行）；
 * 行内编辑（EditRow）不要它 —— 那边标题和正文有各自的位置。
 */
const props = defineProps<{ modelValue: DraftFields; withTitle?: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: DraftFields): void }>()

function set<K extends keyof DraftFields>(key: K, value: DraftFields[K]): void {
  emit('update:modelValue', { ...props.modelValue, [key]: value } as DraftFields)
}

/**
 * 勾上 = 启用期限，并把时间**刷新成此刻**（早先填的默认值可能已经放凉了 ——
 * 表单开着十分钟再勾，填十分钟前的"现在"没意义）。
 * 取消勾选只摘掉开关，值留着：再勾回来界面上不至于变成一片空白。
 */
function toggleDue(on: boolean): void {
  emit('update:modelValue', {
    ...props.modelValue,
    dueOn: on,
    dueAt: on ? nowLocalMinute() : props.modelValue.dueAt,
  })
}

// ── 产出文件 / 目录 ────────────────────────────────────────────────────────
//
// 存进 `files[]` 的是**相对库根的路径 + 正斜杠 + 目录带尾斜杠** ——
// 这个口径是跟 AI 侧 `--files` 对齐的（两边写同一个字段，格式必须一致，
// 否则同一条待办上的产出会一半相对一半绝对）。换算走 `toLibraryRelative`。

const pickerOpen = ref(false)

/**
 * 选一个文件 / 目录挂上去。
 *
 * 目录**末尾补 `/`** 不是装饰：那是"这是个目录"的唯一标记，
 * `artifactIcon()` 认它、AI 侧读取也认它，少了就当文件处理。
 */
async function addArtifact(kind: 'file' | 'dir'): Promise<void> {
  const picked = kind === 'dir' ? await tauriApi.pickDirectory() : await tauriApi.pickFile()
  if (picked === null) return
  const rel = toLibraryRelative(picked, configStore.cfg.library_root)
  if (rel.length === 0) return
  const path = kind === 'dir' ? withDirSlash(rel) : rel
  if (props.modelValue.files.includes(path)) return
  set('files', [...props.modelValue.files, path])
}

function removeArtifact(path: string): void {
  set('files', props.modelValue.files.filter((p) => p !== path))
}

function onPickRelates(ids: string[]): void {
  pickerOpen.value = false
  const next = [...props.modelValue.relates]
  for (const id of ids) if (!next.includes(id)) next.push(id)
  set('relates', next)
}

function removeRelate(id: string): void {
  set('relates', props.modelValue.relates.filter((r) => r !== id))
}
</script>

<template>
  <div class="dfe">
    <template v-if="props.withTitle">
      <div class="dfe-row">
        <span class="dfe-label">标题</span>
        <input
          class="dfe-input"
          type="text"
          placeholder="要办的事"
          spellcheck="false"
          :value="modelValue.title"
          @input="set('title', ($event.target as HTMLInputElement).value)"
        />
      </div>
      <div class="dfe-row">
        <span class="dfe-label">正文</span>
        <textarea
          class="dfe-input dfe-area"
          rows="2"
          placeholder="底稿 / 备注（可空）"
          :value="modelValue.detail"
          @input="set('detail', ($event.target as HTMLTextAreaElement).value)"
        />
      </div>
      <!-- 结论也放进完整表单：行内编辑一直有这一栏，底部那张"完整"表单反而没有，
           结果是新建时写不了结论，想补只能事后点 ✎ -->
      <div class="dfe-row">
        <span class="dfe-label">结论</span>
        <input
          class="dfe-input"
          type="text"
          placeholder="一句话结论（可空；空着卡片上就取正文第一段）"
          spellcheck="false"
          :value="modelValue.summary"
          @input="set('summary', ($event.target as HTMLInputElement).value)"
        />
      </div>
    </template>

    <div class="dfe-row">
      <span class="dfe-label">领域</span>
      <div class="dfe-chips">
        <button
          v-for="d in domainOptions"
          :key="d"
          type="button"
          class="dfe-chip"
          :class="{ on: modelValue.domain === d }"
          @click="set('domain', d)"
        >
          {{ domainName(d) }}
        </button>
      </div>
    </div>

    <div class="dfe-row">
      <span class="dfe-label">权重</span>
      <div class="dfe-chips">
        <button
          v-for="p in PRIORITY_OPTIONS"
          :key="p"
          type="button"
          class="dfe-chip"
          :class="[`w-${p}`, { on: modelValue.priority === p }]"
          @click="set('priority', p)"
        >
          {{ PRIORITY_LABELS[p] }}
        </button>
      </div>
    </div>

    <div class="dfe-row">
      <span class="dfe-label">归属</span>
      <div class="dfe-chips">
        <button
          v-for="o in OWNER_OPTIONS"
          :key="o"
          type="button"
          class="dfe-chip"
          :class="{ on: modelValue.owner === o }"
          @click="set('owner', o)"
        >
          {{ OWNER_LABELS[o] }}
        </button>
      </div>
    </div>

    <div class="dfe-row">
      <span class="dfe-label">状态</span>
      <div class="dfe-chips">
        <button
          v-for="s in STATUS_OPTIONS"
          :key="s"
          type="button"
          class="dfe-chip"
          :class="{ on: modelValue.status === s }"
          @click="set('status', s)"
        >
          {{ STATUS_ACTION_LABELS[s] }}
        </button>
      </div>
    </div>

    <div class="dfe-row">
      <span class="dfe-label">期限</span>
      <!-- 默认**不勾**：待办本来就该"没有期限"居多，不能因为输入框里填着此刻
           就悄悄给每条都挂上一个期限。勾上才写进去，且填的是勾的那一刻 -->
      <label
        class="dfe-due-on"
        :class="{ on: modelValue.dueOn }"
        title="不勾 = 这条待办没有期限；勾上默认填此刻，可以再改"
      >
        <input
          type="checkbox"
          :checked="modelValue.dueOn"
          @change="toggleDue(($event.target as HTMLInputElement).checked)"
        />
        <span>{{ modelValue.dueOn ? '有期限' : '无期限' }}</span>
      </label>
      <input
        class="dfe-input"
        type="datetime-local"
        :disabled="!modelValue.dueOn"
        :value="modelValue.dueAt"
        @input="set('dueAt', ($event.target as HTMLInputElement).value)"
      />
    </div>

    <div class="dfe-row">
      <span class="dfe-label">链接</span>
      <input
        class="dfe-input"
        type="text"
        placeholder="相关链接（可空）"
        spellcheck="false"
        :value="modelValue.link"
        @input="set('link', ($event.target as HTMLInputElement).value)"
      />
    </div>

    <!-- 产出：挂的是"这件事交出去的东西"。点 chip = 摘掉（不是打开）——
         要打开的话外面那张卡片上已经有产出 chip 了，那里才是打开的地方，
         编辑态里手滑点到就跳走反而更烦 -->
    <div class="dfe-row dfe-row-top">
      <span class="dfe-label">产出</span>
      <div class="dfe-stack">
        <div v-if="modelValue.files.length > 0" class="dfe-chips">
          <button
            v-for="p in modelValue.files"
            :key="p"
            type="button"
            class="dfe-file"
            :title="`点一下摘掉：${p}`"
            @click="removeArtifact(p)"
          >
            <span class="dfe-file-icon" aria-hidden="true">{{ artifactIcon(p) }}</span>
            <span class="dfe-file-name">{{ baseName(p) }}</span>
            <X :size="10" :stroke-width="2.6" />
          </button>
        </div>
        <div class="dfe-chips">
          <button type="button" class="dfe-add" @click="addArtifact('file')">加文件</button>
          <button type="button" class="dfe-add" @click="addArtifact('dir')">加目录</button>
        </div>
      </div>
    </div>

    <div class="dfe-row dfe-row-top">
      <span class="dfe-label">关联</span>
      <div class="dfe-stack">
        <div v-if="modelValue.relates.length > 0" class="dfe-chips">
          <button
            v-for="r in modelValue.relates"
            :key="r"
            type="button"
            class="dfe-relate"
            :title="`点一下摘掉：${r}`"
            @click="removeRelate(r)"
          >
            {{ shortId(r) }}
            <X :size="10" :stroke-width="2.6" />
          </button>
        </div>
        <div class="dfe-chips">
          <button type="button" class="dfe-add" @click="pickerOpen = true">关联待办</button>
        </div>
      </div>
    </div>

    <!-- 浮层自己 Teleport 到 body，挂在哪儿都行 -->
    <TaskPicker
      :open="pickerOpen"
      multiple
      title="关联待办"
      :exclude-ids="modelValue.relates"
      @pick="onPickRelates"
      @close="pickerOpen = false"
    />
  </div>
</template>

<style scoped>
.dfe {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.dfe-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.dfe-label {
  flex: none;
  width: 30px;
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
}

.dfe-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.dfe-chip {
  padding: 2px 8px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
  line-height: calc(20.4px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.dfe-chip:hover {
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}

.dfe-chip.on {
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
  font-weight: 600;
}

/* 权重选中的颜色跟卡片左侧色条一致，选完就知道这条会是什么颜色 */
.dfe-chip.on.w-high {
  border-color: var(--xd-w-high);
  background: color-mix(in srgb, var(--xd-w-high) 13%, transparent);
  color: var(--xd-w-high);
}

.dfe-chip.on.w-mid {
  border-color: var(--xd-w-mid);
  background: color-mix(in srgb, var(--xd-w-mid) 13%, transparent);
  color: var(--xd-w-mid);
}

.dfe-chip.on.w-low {
  border-color: var(--xd-w-low);
  background: color-mix(in srgb, var(--xd-w-low) 15%, transparent);
  color: var(--xd-text-sub);
}

.dfe-input {
  flex: 1;
  min-width: 0;
  padding: 4px 8px;
  border: 1px solid var(--xd-border);
  border-radius: 7px;
  background: var(--xd-card);
  outline: none;
  font-size: calc(13.8px * var(--xd-font-scale));
  color: var(--xd-text);
  transition: border-color 0.15s var(--xd-ease);
}

.dfe-input:focus {
  border-color: var(--xd-accent);
}

/* 没勾期限 = 这条待办没有期限，输入框只是"默认值预览"，压暗表示不算数 */
.dfe-input:disabled {
  opacity: 0.45;
  color: var(--xd-text-dim);
}

/* 期限的开关：勾上才变强调色，跟旁边压暗/点亮的时间框对上 */
.dfe-due-on {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--xd-text-dim);
  font-size: calc(12.4px * var(--xd-font-scale));
  cursor: pointer;
  user-select: none;
  transition: color 0.14s var(--xd-ease);
}

.dfe-due-on input {
  width: calc(13px * var(--xd-font-scale));
  height: calc(13px * var(--xd-font-scale));
  margin: 0;
  accent-color: var(--xd-accent);
  cursor: pointer;
}

.dfe-due-on:hover {
  color: var(--xd-text-sub);
}

.dfe-due-on.on {
  color: var(--xd-accent);
}

.dfe-area {
  resize: vertical;
  line-height: 1.55;
  font-family: inherit;
}

/* ── 产出 / 关联两行 ─────────────────────────────────────────────────────
   带 chips 的行：标签顶对齐，右边一列装"已挂的 + 添加按钮"。
   行内编辑时表单很窄，挂多了会把整张表单撑破 —— 所以给个高度上限，超出内部滚。 */

.dfe-row-top {
  align-items: flex-start;
}

.dfe-row-top .dfe-label {
  padding-top: 3px;
}

.dfe-stack {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: calc(96px * var(--xd-font-scale));
  overflow-y: auto;
}

.dfe-file,
.dfe-relate {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  padding: 2px 7px;
  border: 1px solid var(--xd-border);
  border-radius: 999px;
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: calc(19px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

/* 点 chip = 摘掉：悬停转成警示红，别让人以为是"打开" */
.dfe-file:hover,
.dfe-relate:hover {
  border-color: var(--xd-red);
  color: var(--xd-red);
}

.dfe-file-icon {
  flex: none;
  font-size: calc(11.6px * var(--xd-font-scale));
  line-height: 1;
}

/* 文件名可能很长，压缩省略 —— 完整路径在 title 里 */
.dfe-file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dfe-relate {
  font-family: 'Cascadia Code', Consolas, monospace;
}

.dfe-add {
  padding: 2px 9px;
  border: 1px dashed var(--xd-border);
  border-radius: 999px;
  background: transparent;
  color: var(--xd-text-dim);
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: calc(19px * var(--xd-font-scale));
  transition: all 0.14s var(--xd-ease);
}

.dfe-add:hover {
  border-style: solid;
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}
</style>
