<script setup lang="ts">
// 打开产出：md 用什么打开、白名单目录。
//
// 白名单是**安全兜底**：点产出文件时只认这些目录下的路径，
// 免得待办里被人塞一句 `C:\Windows\...` 就跟着去开。
import { inject } from 'vue'
import { Trash2 } from 'lucide-vue-next'
import { tauriApi } from '../../api/tauri'
import { configStore, saveConfig } from '../../stores/config'

const showToast = inject<(s: string) => void>('showToast', () => {})

async function setMdMode(v: string): Promise<void> {
  await saveConfig({ md_open_mode: v })
}

async function pickPreviewExe(): Promise<void> {
  const picked = await tauriApi.pickFile(['exe'])
  if (!picked) return
  await saveConfig({ preview_exe: picked })
  showToast('已设置 md 查看器')
}

async function addRoot(): Promise<void> {
  const picked = await tauriApi.pickDirectory()
  if (!picked) return
  const roots = configStore.cfg.open_roots
  if (roots.includes(picked)) return
  await saveConfig({ open_roots: [...roots, picked] })
  showToast('已加入白名单')
}

async function removeRoot(root: string): Promise<void> {
  await saveConfig({ open_roots: configStore.cfg.open_roots.filter((r) => r !== root) })
}
</script>

<template>
  <!-- 打开方式 -->
  <section id="sv-sec-mode" class="sv-sec">
    <h3 class="sv-sec-title">打开方式</h3>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">md / txt 用什么打开</span>
        <span class="setting-desc">
          「MD-Preview」交给专用查看器；「系统默认」交给系统关联的程序
        </span>
      </div>
      <div class="sv-seg">
        <button
          type="button"
          :class="{ on: configStore.cfg.md_open_mode === 'preview' }"
          @click="setMdMode('preview')"
        >
          MD-Preview
        </button>
        <button
          type="button"
          :class="{ on: configStore.cfg.md_open_mode === 'system' }"
          @click="setMdMode('system')"
        >
          系统默认
        </button>
      </div>
    </div>

    <div class="setting-row" style="flex-direction: column; align-items: stretch">
      <div class="setting-info">
        <span class="setting-name">MD-Preview 位置</span>
        <span class="setting-desc">不在了会退回「打开所在文件夹」</span>
      </div>
      <div class="sv-path">
        <span class="sv-path-text xd-select">{{ configStore.cfg.preview_exe }}</span>
        <div class="sv-path-actions">
          <button type="button" class="sv-btn" @click="pickPreviewExe">浏览…</button>
        </div>
      </div>
    </div>

  </section>

  <!-- 白名单 -->
  <section id="sv-sec-roots" class="sv-sec">
    <h3 class="sv-sec-title">可打开的目录</h3>

    <div class="setting-row" style="flex-direction: column; align-items: stretch">
      <div class="setting-info">
        <span class="setting-name">允许打开的目录</span>
        <span class="setting-desc">只有白名单里的产出会被打开，其余忽略</span>
      </div>
      <div class="sv-list">
        <div v-for="r in configStore.cfg.open_roots" :key="r" class="sv-list-item">
          <span class="sv-list-text">{{ r }}</span>
          <button type="button" class="sv-list-del" aria-label="移除" @click="removeRoot(r)">
            <Trash2 :size="12" :stroke-width="2" />
          </button>
        </div>
        <div class="sv-path-actions" style="margin-top: 4px">
          <button type="button" class="sv-btn" @click="addRoot">添加目录…</button>
        </div>
      </div>
    </div>
  </section>
</template>
