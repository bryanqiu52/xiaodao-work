<script setup lang="ts">
// 编号 chip：显示截断形态，点一下复制完整编号。
// 复制要兜底 execCommand —— 非安全上下文（http 调试）里 clipboard API 不可用。
import { shortId } from '../core/view'

const props = defineProps<{ id: string }>()
const emit = defineEmits<{ (e: 'copied', id: string): void }>()

async function copy(): Promise<void> {
  try {
    await navigator.clipboard.writeText(props.id)
  } catch {
    const ta = document.createElement('textarea')
    ta.value = props.id
    ta.style.position = 'fixed'
    ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    try {
      document.execCommand('copy')
    } catch {
      return
    } finally {
      ta.remove()
    }
  }
  emit('copied', props.id)
}
</script>

<template>
  <button type="button" class="idchip" :title="`点击复制编号 ${id}`" @click.stop="copy">
    {{ shortId(id) }}
  </button>
</template>

<style scoped>
.idchip {
  padding: 1px 6px;
  border: 1px solid var(--xd-border);
  border-radius: 5px;
  background: var(--xd-card-sub);
  color: var(--xd-text-dim);
  font-size: calc(13.2px * var(--xd-font-scale));
  font-family: 'Cascadia Code', Consolas, monospace;
  line-height: calc(19.2px * var(--xd-font-scale));
  transition: color 0.15s var(--xd-ease), border-color 0.15s var(--xd-ease);
}

.idchip:hover {
  color: var(--xd-accent);
  border-color: var(--xd-accent);
}
</style>
