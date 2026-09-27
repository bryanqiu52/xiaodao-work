<script setup lang="ts">
// 底部常驻输入：回车即记（只填标题，其余用默认值）；点 ＋ 升起完整表单。
//
// 表单是**居中的磨砂浮层**（不是贴底，也不是内联撑开），照原插件：
// 内联会把列表往上挤，窗口一矮就什么都看不见；浮层独立于列表，位置永远稳定。
//
// 提交后清标题 / 正文 / 链接 / 期限 / 产出 / 关联，**保留领域 / 权重 / 归属 / 状态** ——
// 连着记一批同类的事不用每次重选；而产出和关联是"这一条"的东西，留着会被下一条继承。
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { Plus } from 'lucide-vue-next'
import DraftFieldsEditor from './DraftFieldsEditor.vue'
import { emptyDraft, nowLocalMinute, type DraftFields } from '../core/draft'

const emit = defineEmits<{
  (e: 'add', draft: DraftFields): void
  (e: 'result', text: string): void
}>()

const draft = ref<DraftFields>(emptyDraft())
const expanded = ref(false)
const inputRef = ref<HTMLInputElement | null>(null)

function focusInput(): void {
  void nextTick(() => inputRef.value?.focus())
}

defineExpose({ focusInput })

function closeSheet(): void {
  expanded.value = false
}

function submit(): void {
  const title = draft.value.title.trim()
  if (title.length === 0) {
    emit('result', '先写一句要办的事')
    return
  }
  emit('add', { ...draft.value, title })
  // 保留领域 / 权重 / 归属 / 状态；期限回到"不勾 + 默认此刻"。
  // **产出和关联必须清掉** —— 它们说的是"这一条"的东西，
  // 留着会被下一条静默继承，等于给不相干的待办挂上别人的产出文件。
  draft.value = {
    ...draft.value,
    title: '',
    detail: '',
    summary: '',
    dueAt: nowLocalMinute(),
    dueOn: false,
    link: '',
    files: [],
    relates: [],
  }
  closeSheet()
  focusInput()
}

/** 清空：连那些"保留"的字段也一起重置，并收起表单 */
function clearDraft(): void {
  draft.value = emptyDraft()
  closeSheet()
  focusInput()
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Enter' && !e.shiftKey && !expanded.value) {
    e.preventDefault()
    submit()
  }
}

function onEsc(e: KeyboardEvent): void {
  if (e.key === 'Escape') closeSheet()
}

// Esc 只在表单升起时监听，收起状态下不抢键
watch(expanded, (on) => {
  if (on) document.addEventListener('keydown', onEsc)
  else document.removeEventListener('keydown', onEsc)
})
onBeforeUnmount(() => document.removeEventListener('keydown', onEsc))
</script>

<template>
  <div class="cp">
    <div class="cp-line">
      <input
        ref="inputRef"
        v-model="draft.title"
        class="cp-input"
        type="text"
        placeholder="写一句要办的事，回车就记下"
        spellcheck="false"
        @keydown="onKeydown"
        @keydown.ctrl.enter="submit"
        @keydown.meta.enter="submit"
      />
      <button
        type="button"
        class="cp-plus"
        :class="{ on: expanded }"
        :aria-expanded="expanded"
        :aria-label="expanded ? '收起完整表单' : '展开完整表单'"
        :title="expanded ? '收起完整表单' : '展开完整表单（正文 / 领域 / 归属 / 权重 / 期限 / 链接）'"
        @click="expanded = !expanded"
      >
        <Plus :size="17" :stroke-width="2.4" />
      </button>
      <button type="button" class="cp-send" :disabled="draft.title.trim().length === 0" @click="submit">
        记下
      </button>
    </div>
  </div>

  <!-- 浮层传送到 body：外卡有圆角裁剪，卡片 hover 还有 transform，
       留在这里会被裁掉、或者位置算错（fixed 遇到有 transform 的祖先就不再相对视口了） -->
  <Teleport to="body">
    <template v-if="expanded">
      <div class="cp-scrim" @click="closeSheet" />
      <div class="cp-sheet" role="dialog" aria-label="完整表单">
        <div class="cp-sheet-head">
          <span class="cp-sheet-title">完整表单（可留空）</span>
        </div>
        <div class="cp-sheet-body">
          <DraftFieldsEditor v-model="draft" with-title />
        </div>
        <div class="cp-sheet-actions">
          <span class="cp-hint">Ctrl+Enter 也能添加</span>
          <span class="cp-spacer" />
          <button type="button" class="cp-btn" title="清空并收起" @click="clearDraft">清空</button>
          <button type="button" class="cp-btn primary" @click="submit">添加</button>
        </div>
      </div>
    </template>
  </Teleport>
</template>

<style scoped>
.cp {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 底部这一行没有"容器"：输入框自己就是那颗灰底胶囊，右边跟一颗实色「＋」。 */
.cp-line {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* 灰底、无描边；聚焦才"亮起"成卡片底 + 品牌描边 + 光环，提示正在输入 */
.cp-input {
  flex: 1;
  min-width: 0;
  height: calc(36px * var(--xd-font-scale));
  padding: 0 12px;
  border: 1px solid transparent;
  border-radius: 10px;
  background: var(--xd-card-sub);
  outline: none;
  font-size: calc(15.6px * var(--xd-font-scale));
  color: var(--xd-text);
  transition:
    background 0.16s var(--xd-ease),
    border-color 0.16s var(--xd-ease),
    box-shadow 0.16s var(--xd-ease);
}

.cp-input::placeholder {
  color: var(--xd-text-dim);
}

.cp-input:focus {
  background: var(--xd-card);
  border-color: var(--xd-accent);
  box-shadow: 0 0 0 3px var(--xd-accent-soft);
}

/* 「＋」是这颗主按钮：实色填充 + 圆角方形 + 投影。
   展开时转 45° 变「×」——同一个位置表达"再点一下就收" */
.cp-plus {
  flex: none;
  display: grid;
  place-items: center;
  width: calc(36px * var(--xd-font-scale));
  height: calc(36px * var(--xd-font-scale));
  border: 0;
  border-radius: 10px;
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  cursor: pointer;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.32);
  transition:
    transform 0.18s var(--xd-ease),
    filter 0.16s var(--xd-ease);
}

.cp-plus:hover {
  filter: brightness(1.08);
}

.cp-plus.on {
  transform: rotate(45deg);
}

/* 「记下」是次要出口（回车也能提交），所以走描边，不跟「＋」抢主按钮的位置 */
.cp-send {
  flex: none;
  height: calc(36px * var(--xd-font-scale));
  padding: 0 12px;
  border: 1px solid var(--xd-border);
  border-radius: 10px;
  background: transparent;
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  cursor: pointer;
  transition: all 0.15s var(--xd-ease);
}

.cp-send:hover:not(:disabled) {
  border-color: var(--xd-accent);
  color: var(--xd-accent);
}

.cp-send:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* ── 完整表单：居中 + 磨砂玻璃 ───────────────────────────────────────────── */

.cp-scrim {
  position: fixed;
  inset: 0;
  z-index: 65;
  /* 遮罩是铺满视口的，而窗口四角是**透明**的（圆角由 .win 裁出来）——
     不给遮罩同样的圆角，这两块就会像补丁一样糊在窗外那四个角上。 */
  border-radius: var(--xd-window-radius);
  /* 别压太黑：背后那层列表得看得见轮廓，模糊才有东西可糊。
     压到全黑就是在模糊一块纯色，磨砂感会彻底消失。 */
  background: rgba(0, 0, 0, 0.28);
  animation: cp-fade 0.18s ease-out;
}

@keyframes cp-fade {
  from {
    opacity: 0;
  }
}

/* `backdrop-filter` 是磨砂的来源：身后的列表被模糊成一片，浮层像块玻璃浮在上面。
   前提是**底色得半透明**（见 --xd-sheet-bg），实底加模糊是白费 */
.cp-sheet {
  position: fixed;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  z-index: 70;
  display: flex;
  flex-direction: column;
  width: min(420px, calc(100vw - 28px));
  max-height: calc(100vh - 28px);
  padding: 14px 16px 16px;
  border: 1px solid var(--xd-border);
  border-radius: 16px;
  background: var(--xd-sheet-bg);
  /* 同上：与「筛选」「删除确认」共用一组变量 */
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  box-shadow:
    0 10px 30px rgba(0, 0, 0, 0.5),
    /* 顶部一道内高光：玻璃的"厚度"就靠它，没有这行只会像半透明塑料 */
    inset 0 1px 0 rgba(255, 255, 255, 0.1);
  animation: cp-pop 0.18s var(--xd-ease);
}

/* 关键帧里必须把 translate(-50%,-50%) 一起写上：
   animation 会整个接管 transform，只写 scale 会把居中那部分吃掉、弹窗跑到右下角 */
@keyframes cp-pop {
  from {
    transform: translate(-50%, -50%) scale(0.96);
    opacity: 0;
  }
}

.cp-sheet-head {
  flex: none;
  margin-bottom: 8px;
}

.cp-sheet-title {
  font-size: calc(14.4px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text-sub);
}

.cp-sheet-body {
  flex: 1;
  min-height: 0;
  display: grid;
  gap: 8px;
  align-content: start;
  overflow: auto;
}

.cp-sheet-actions {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 10px;
}

.cp-hint {
  color: var(--xd-text-dim);
  font-size: calc(12.6px * var(--xd-font-scale));
}

.cp-spacer {
  flex: 1;
}

.cp-btn {
  padding: 5px 12px;
  border: 1px solid var(--xd-border);
  border-radius: 8px;
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(13.2px * var(--xd-font-scale));
  cursor: pointer;
  transition: all 0.15s var(--xd-ease);
}

.cp-btn:hover {
  color: var(--xd-accent);
  border-color: var(--xd-accent);
}

.cp-btn.primary {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.cp-btn.primary:hover {
  border-color: var(--xd-accent-hover);
  background: var(--xd-accent-hover);
  color: var(--xd-accent-text);
}
</style>
