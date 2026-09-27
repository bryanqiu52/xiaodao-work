<script setup lang="ts">
// 删除二次确认：**从画面中间弹出来**（跟「完整表单」同一个浮层样式），
// 5 秒没人应答自动撤回。
//
// 为什么要这道确认：软删除只是打时间戳、记录还在，但**列表上立刻就看不见了**，
// 和"完成"在视觉上几乎没差别。误点一下，得翻 JSON 才找得回来。
//
// 为什么改成居中浮层而不是就地盖在卡片上：盖在卡片上时，被盖住的正是"要删的那条"，
// 想再确认一眼内容反而看不见了；居中弹出来，卡片原样留在身后，看得清要删的是什么。
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { CONFIRM_MS } from '../core/constants'

const props = defineProps<{ title: string }>()
const emit = defineEmits<{ (e: 'confirm'): void; (e: 'cancel'): void }>()

const left = ref(Math.ceil(CONFIRM_MS / 1000))
const cancelBtn = ref<HTMLButtonElement | null>(null)
let timer: number | undefined

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('cancel')
}

onMounted(() => {
  // 焦点默认落在「取消」上：删除这种操作，什么都不做就该是安全的那个结果
  cancelBtn.value?.focus()
  timer = window.setInterval(() => {
    left.value -= 1
    if (left.value <= 0) emit('cancel')
  }, 1000)
  window.addEventListener('keydown', onKey)
})

onBeforeUnmount(() => {
  if (timer !== undefined) window.clearInterval(timer)
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <!-- 传送到 body：卡片 hover 有 transform、外卡还有圆角裁剪 ——
       留在卡片里的话 fixed 的基准会变成卡片（位置跟着卡片跑），也会被裁掉 -->
  <Teleport to="body">
    <div class="rc-scrim" @click="emit('cancel')" />
    <div class="rc-sheet" role="alertdialog" aria-label="移出列表确认" @click.stop>
      <p class="rc-title">移出列表？</p>
      <p class="rc-desc">记录会留在文件里（软删除），但列表上看不见了。</p>
      <p class="rc-name">{{ props.title }}</p>
      <div class="rc-actions">
        <button ref="cancelBtn" type="button" class="rc-btn ghost" @click="emit('cancel')">
          取消（{{ left }}）
        </button>
        <button type="button" class="rc-btn danger" @click="emit('confirm')">移出</button>
      </div>
      <p class="rc-hint">点遮罩或按 Esc 也是取消</p>
    </div>
  </Teleport>
</template>

<style scoped>
/* 遮罩铺满视口。窗口四角是透明的（圆角由 .win 裁出来），
   不给遮罩同样的圆角，它就会像补丁一样糊在窗外那四个角上 */
.rc-scrim {
  position: fixed;
  inset: 0;
  z-index: 65;
  border-radius: var(--xd-window-radius);
  background: rgba(0, 0, 0, 0.35);
}

/* 浮层底色必须是"实"的（--xd-sheet-bg）：确认框要你看清再按，
   卡片本身可能被调透明、沉浸模式下更是半透 —— 跟着透就叠字了 */
.rc-sheet {
  position: fixed;
  left: 50%;
  top: 50%;
  translate: -50% -50%;
  z-index: 70;
  width: min(340px, calc(100vw - 40px));
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px 18px 14px;
  border-radius: var(--xd-radius-lg);
  background: var(--xd-sheet-bg);
  /* 模糊参数跟「完整表单」「筛选」共用同一组变量 */
  -webkit-backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  backdrop-filter: blur(var(--xd-sheet-blur)) saturate(var(--xd-sheet-saturate));
  border: 1px solid var(--xd-border);
  box-shadow: var(--xd-shadow-pop);
  animation: rc-in 0.16s var(--xd-ease);
}

@keyframes rc-in {
  from {
    opacity: 0;
    translate: -50% -47%;
  }
}

.rc-title {
  margin: 0;
  font-size: calc(15.6px * var(--xd-font-scale));
  font-weight: 600;
  color: var(--xd-text);
}

.rc-desc {
  margin: 0;
  font-size: calc(13.2px * var(--xd-font-scale));
  line-height: 1.55;
  color: var(--xd-text-sub);
}

/* 被删的那条单独成行：浮层不再盖住卡片，这里把标题原样复述一遍 */
.rc-name {
  margin: 0;
  padding: 7px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--xd-text) 6%, transparent);
  font-size: calc(13px * var(--xd-font-scale));
  line-height: 1.45;
  color: var(--xd-text);
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.rc-actions {
  display: flex;
  gap: 8px;
  margin-top: 2px;
}

.rc-btn {
  flex: 1;
  padding: 8px 10px;
  border-radius: 7px;
  border: 1px solid var(--xd-border);
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(14.4px * var(--xd-font-scale));
  cursor: pointer;
  transition:
    border-color 0.15s var(--xd-ease),
    color 0.15s var(--xd-ease),
    background 0.15s var(--xd-ease);
}

.rc-btn.ghost:hover {
  border-color: var(--xd-text-dim);
  color: var(--xd-text);
}

.rc-btn.danger {
  border-color: color-mix(in srgb, var(--xd-red) 45%, transparent);
  background: color-mix(in srgb, var(--xd-red) 12%, transparent);
  color: var(--xd-red);
  font-weight: 600;
}

.rc-btn.danger:hover {
  background: color-mix(in srgb, var(--xd-red) 20%, transparent);
}

.rc-hint {
  margin: 0;
  font-size: calc(11.6px * var(--xd-font-scale));
  color: var(--xd-text-dim);
}
</style>
