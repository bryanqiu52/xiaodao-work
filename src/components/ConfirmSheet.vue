<script setup lang="ts">
// 通用确认浮层：**应用里所有"问一句再动手"的地方都走它**（见 useConfirm.ts）。
//
// 视觉上与 `RemoveConfirm.vue`（列表里那条删除确认）保持同一套 —— 同一组
// `--xd-sheet-*` 变量、同样的圆角与投影。两个都用，是为了各管一摊：
//   - 这个：设置页/记账那种**跨页面的一次性询问**，由 `askConfirm()` 唤起；
//   - `RemoveConfirm`：**列表行内**的删除确认，带"5 秒没人应答自动撤回"，
//     它是那个列表自己的交互，独立成件更清楚。
// 两者长得一样，用户看不出是两个组件。
//
// 三条交互规矩（跟 RemoveConfirm 一致，别改回去）：
//   1. **焦点默认落在「取消」**上 —— 这种框多半是在问"要不要做一件不能撤销的事"，
//      什么都不做就该是安全的那个结果；
//   2. **只认 Esc = 取消**，不绑 Enter 确认 —— 危险操作不该被一次顺手回车触发；
//   3. 遮罩点击也是取消。
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle } from 'lucide-vue-next'
import { confirmState, settle } from '../composables/useConfirm'

const opts = computed(() => confirmState.opts)
const cancelBtn = ref<HTMLButtonElement | null>(null)

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') settle(false)
}

// 只在开着的时候挂键盘监听：关着的时候没必要跟着每一次按键跑
watch(
  () => confirmState.open,
  async (open) => {
    if (!open) return
    await nextTick()
    cancelBtn.value?.focus()
  },
)

onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <!-- 传送到 body：卡片 hover 有 transform、外卡还有圆角裁剪 ——
       留在卡片里的话 fixed 的基准会变成卡片，位置跟着跑，还会被裁掉 -->
  <Teleport to="body">
    <template v-if="confirmState.open && opts">
      <div class="confirm-scrim" @click="settle(false)" />
      <div class="confirm-sheet" role="alertdialog" :aria-label="opts.title" @click.stop>
        <p class="confirm-title">
          <AlertTriangle v-if="opts.danger" :size="14" :stroke-width="2.2" class="confirm-icon" />
          {{ opts.title }}
        </p>

        <p v-for="(line, i) in opts.desc ?? []" :key="`d${i}`" class="confirm-desc">{{ line }}</p>

        <!-- 原样复述：路径 / 备份名 / 被删的那条。等宽 + 单独一块底，混不进正文里 -->
        <p v-if="opts.note" class="confirm-note">{{ opts.note }}</p>

        <ul v-if="(opts.facts ?? []).length > 0" class="confirm-facts">
          <li v-for="f in opts.facts" :key="f">{{ f }}</li>
        </ul>

        <p v-if="opts.warning" class="confirm-warn">{{ opts.warning }}</p>

        <div class="confirm-actions">
          <button ref="cancelBtn" type="button" class="confirm-btn ghost" @click="settle(false)">
            {{ opts.cancelText ?? '取消' }}
          </button>
          <button
            type="button"
            class="confirm-btn"
            :class="opts.danger ? 'danger' : 'primary'"
            @click="settle(true)"
          >
            {{ opts.confirmText ?? '确定' }}
          </button>
        </div>
        <p class="confirm-hint">点遮罩或按 Esc 也是取消</p>
      </div>
    </template>
  </Teleport>
</template>

<style scoped>
/* 遮罩铺满视口。窗口四角是透明的（圆角由 .win 裁出来），
   不给遮罩同样的圆角，它就会像补丁一样糊在窗外那四个角上 */
.confirm-scrim {
  position: fixed;
  inset: 0;
  z-index: 85;
  border-radius: var(--xd-window-radius);
  background: rgba(0, 0, 0, 0.35);
}

/* 底色必须是"实"的（--xd-sheet-bg）：确认框要你看清再按，
   卡片本身可能被调透明、沉浸模式下更是半透 —— 跟着透就叠字了 */
.confirm-sheet {
  position: fixed;
  left: 50%;
  top: 50%;
  translate: -50% -50%;
  z-index: 90;
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: min(380px, calc(100vw - 32px));
  max-height: calc(100vh - 40px);
  overflow-y: auto;
  padding: 16px 18px 14px;
  border-radius: var(--xd-radius-lg);
  background: var(--xd-sheet-bg);
  /* 模糊参数跟「完整表单」「筛选」共用同一组变量 */
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  border: 1px solid var(--xd-border);
  box-shadow: var(--xd-shadow-pop);
  animation: confirm-in 0.16s var(--xd-ease);
}

@keyframes confirm-in {
  from {
    opacity: 0;
    translate: -50% -47%;
  }
}

.confirm-title {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: calc(15.6px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
}

.confirm-icon {
  flex: none;
  color: var(--xd-red);
}

.confirm-desc {
  margin: 0;
  font-size: calc(13.2px * var(--xd-font-scale));
  line-height: 1.55;
  color: var(--xd-text-sub);
}

/* 原样复述的东西：等宽字形，跟正文区分开，一眼知道"动的是这一份" */
.confirm-note {
  margin: 0;
  padding: 7px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--xd-text) 6%, transparent);
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: calc(12.6px * var(--xd-font-scale));
  line-height: 1.5;
  color: var(--xd-text);
  word-break: break-all;
}

/* 逐条要点：原来的系统弹窗里这些是手打的 "  · xxx"，这里交给样式做 */
.confirm-facts {
  margin: 0;
  padding-left: 16px;
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: calc(12.8px * var(--xd-font-scale));
  line-height: 1.55;
  color: var(--xd-text-sub);
}

.confirm-facts li::marker {
  color: var(--xd-text-dim);
}

/* 最后那句提醒：危险操作里最要紧的一行（"能不能后悔"），压红 */
.confirm-warn {
  margin: 2px 0 0;
  padding: 6px 9px;
  border-radius: 7px;
  background: color-mix(in srgb, var(--xd-red) 10%, transparent);
  font-size: calc(12.4px * var(--xd-font-scale));
  line-height: 1.5;
  color: var(--xd-red);
}

.confirm-actions {
  display: flex;
  gap: 8px;
  margin-top: 2px;
}

.confirm-btn {
  flex: 1;
  padding: 8px 10px;
  border-radius: 7px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  transition:
    border-color 0.15s var(--xd-ease),
    color 0.15s var(--xd-ease),
    background 0.15s var(--xd-ease);
}

.confirm-btn.ghost:hover {
  border-color: var(--xd-text-dim);
  color: var(--xd-text);
}

/* 危险操作：红框红字，跟"改个目录"那种普通操作用同一个按钮位置、不同颜色区分 */
.confirm-btn.danger {
  border-color: color-mix(in srgb, var(--xd-red) 45%, transparent);
  background: color-mix(in srgb, var(--xd-red) 12%, transparent);
  color: var(--xd-red);
  font-weight: 600;
}

.confirm-btn.danger:hover {
  background: color-mix(in srgb, var(--xd-red) 20%, transparent);
}

/* 普通操作走强调色（跟界面里其它主按钮一个语言） */
.confirm-btn.primary {
  border-color: var(--xd-accent);
  background: var(--xd-accent);
  color: var(--xd-accent-text);
  font-weight: 600;
}

.confirm-btn.primary:hover {
  background: var(--xd-accent-hover);
}

.confirm-hint {
  margin: 0;
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}
</style>
