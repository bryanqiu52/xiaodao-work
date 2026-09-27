<script setup lang="ts">
// 正文渲染：只认 core/brief.ts 解析出的 token，**绝不用 v-html**。
// 正文里写什么都不该变成能执行的 HTML。
import { computed } from 'vue'
import { parseDetail } from '../core/brief'

const props = defineProps<{ text: string }>()
const tokens = computed(() => parseDetail(props.text))
</script>

<template>
  <span class="dt">
    <template v-for="(tk, i) in tokens" :key="i">
      <code v-if="tk.t === 'code'" class="dt-code">{{ tk.v }}</code>
      <strong v-else-if="tk.t === 'bold'">{{ tk.v }}</strong>
      <em v-else-if="tk.t === 'italic'">{{ tk.v }}</em>
      <a
        v-else-if="tk.t === 'link'"
        class="dt-link"
        :href="tk.href"
        target="_blank"
        rel="noreferrer"
        @click.stop
        >{{ tk.text }}</a
      >
      <template v-else>{{ tk.v }}</template>
    </template>
  </span>
</template>

<style scoped>
.dt {
  white-space: pre-wrap;
  word-break: break-word;
}

.dt-code {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--xd-card-sub);
  border: 1px solid var(--xd-border-soft);
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: 0.92em;
}

.dt-link {
  color: var(--xd-accent);
  text-decoration: none;
  border-bottom: 1px solid transparent;
}

.dt-link:hover {
  border-bottom-color: var(--xd-accent);
}
</style>
