<script setup lang="ts">
// 产出文件 chip。
// 左键 = 打开文件本身（预览/默认程序由后端按白名单定）；**右键 = 弹菜单，点了才动**。
//
// 右键原来是一按就直接打开所在文件夹，改回弹菜单了（原插件 2026-09-23 也是这么定的）：
// 右键这种"顺手一按"的动作不该立刻产生副作用，先给个出口，用户点了才执行。
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { logToBackend, tauriApi } from '../api/tauri'
import { absolutePath, baseName } from '../core/brief'
import { artifactIcon } from '../core/constants'

const props = defineProps<{
  path: string
  libRoot: string
}>()

const emit = defineEmits<{ (e: 'result', text: string): void }>()

/** 菜单的估算尺寸：只用来把它夹在窗口里，不必等它渲染完再量一次 */
const MENU_W = 176
const MENU_H = 44
/** 打不开时 chip 变红多久 */
const FAIL_MS = 2000

const full = computed(() => absolutePath(props.path, props.libRoot))
const name = computed(() => baseName(props.path))
/** 图标按扩展名走（照搬原插件那张表），目录是 📁 */
const icon = computed(() => artifactIcon(props.path))
// 目录的约定是**末尾带斜杠**。别去猜"没扩展名就是目录" —— 产出里有不带扩展名的文件，
// 猜错了菜单文案就反了。（原插件同样只认这个）
const isDir = computed(() => /[\\/]$/.test(props.path))
const failed = ref(false)

/** 右键菜单：null = 没开；开了就记着鼠标落点，菜单摆在那个点附近 */
const menu = ref<{ x: number; y: number } | null>(null)
const menuEl = ref<HTMLElement | null>(null)

const menuLabel = computed(() => (isDir.value ? '打开文件夹' : '打开所在文件夹'))
const tip = computed(() => {
  if (failed.value) return `打不开（可能路径已失效）：${full.value}`
  if (isDir.value) return `打开文件夹：${full.value}\n右键 = 菜单里再点一次`
  return `打开：${full.value}\n右键 = 菜单里打开所在文件夹`
})

async function open(mode: 'file' | 'folder'): Promise<void> {
  const result = await tauriApi.openPath(full.value, mode)
  if (result === 'ok') return
  if (result === 'browser') emit('result', `（浏览器调试）将打开：${name.value}`)
  else if (result === 'blocked') emit('result', '这类文件不代跑，已打开所在文件夹')
  else if (result === 'outside') emit('result', '这个路径不在白名单内，已忽略')
  else if (result === 'missing') emit('result', '文件不在了（可能已移动或改名）')
  // 后端抛回来的失败以前被无声吞掉，表现就是"点了没反应"——必须说出来
  else if (result.startsWith('error:')) emit('result', `打开失败：${result.slice(6)}`)
  // 提示条会滚走，chip 自己红一下才看得清是哪个文件出的问题
  failed.value = true
  window.setTimeout(() => {
    failed.value = false
  }, FAIL_MS)
}

function onContextMenu(event: MouseEvent): void {
  // 右键**只弹菜单，不直接开**。preventDefault 不能少，否则外壳自己的右键菜单会盖上来。
  event.preventDefault()
  event.stopPropagation()
  logToBackend('info', `产出菜单：打开 @${event.clientX},${event.clientY}`)
  menu.value = { x: event.clientX, y: event.clientY }
}

/** 收起菜单。开着的时候记一笔 —— "闪一下"就是开了又立刻被收，日志能看出是谁收的 */
function closeMenu(): void {
  if (menu.value) logToBackend('info', '产出菜单：收起')
  menu.value = null
}

/** 点了菜单里的那一项：先收菜单，再执行 —— 顺序反了菜单会挡住结果提示 */
function pickFolder(): void {
  logToBackend('info', `产出菜单：点了「${menuLabel.value}」`)
  menu.value = null
  void open('folder')
}

const place = computed(() => {
  const x = menu.value?.x ?? 0
  const y = menu.value?.y ?? 0
  // 夹在窗口内：菜单位置是 fixed，跑到窗口外面就点不着了
  const maxLeft = Math.max(8, window.innerWidth - MENU_W - 8)
  const maxTop = Math.max(8, window.innerHeight - MENU_H - 8)
  return {
    left: `${Math.max(8, Math.min(x, maxLeft))}px`,
    top: `${Math.max(8, Math.min(y, maxTop))}px`,
    width: `${MENU_W}px`,
  }
})

function onDocDown(event: MouseEvent): void {
  // **只认左键**。右键本身会派发一次 mousedown，不区分按键的话菜单刚开就被它关掉
  // —— 表现就是"闪一下"。
  if (event.button !== 0) return
  // 点在菜单自己身上不算"点别处"
  if (menuEl.value?.contains(event.target as Node)) return
  closeMenu()
}
function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') closeMenu()
}
function onScroll(): void {
  // 菜单是 fixed 的，列表一滚它就飘在原地不跟着走 —— 直接收掉最干净
  if (menu.value) closeMenu()
}

watch(menu, async (open) => {
  if (!open) {
    document.removeEventListener('mousedown', onDocDown)
    document.removeEventListener('keydown', onKey)
    window.removeEventListener('scroll', onScroll, true)
    return
  }
  document.addEventListener('mousedown', onDocDown)
  document.addEventListener('keydown', onKey)
  window.addEventListener('scroll', onScroll, true)
  // 渲染完量一次实际落点：菜单"看不见"时要能分清是没渲染、还是渲染到窗口外面去了
  await nextTick()
  const rect = menuEl.value?.getBoundingClientRect()
  logToBackend(
    'info',
    rect
      ? `产出菜单：已渲染 ${Math.round(rect.left)},${Math.round(rect.top)} ${Math.round(rect.width)}x${Math.round(rect.height)} 视口 ${window.innerWidth}x${window.innerHeight}`
      : '产出菜单：没渲染出来',
  )
})

onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocDown)
  document.removeEventListener('keydown', onKey)
  window.removeEventListener('scroll', onScroll, true)
})
</script>

<template>
  <!-- 菜单挂在**外层 span** 上，不能塞进 button：按钮套按钮是非法嵌套，
       而且菜单项的点击会冒泡到 chip 自己，变成"点菜单 = 又点了一次左键" -->
  <span class="art-cell">
    <button
      type="button"
      class="art"
      :class="{ failed }"
      :title="tip"
      @click.stop="open('file')"
      @contextmenu="onContextMenu"
    >
      <span class="art-icon" aria-hidden="true">{{ icon }}</span>
      <span class="art-name">{{ name }}</span>
    </button>

  </span>

  <!-- 菜单必须传送出去（`Teleport to="body"`），不能留在 chip 里面。
       卡片 hover 时有 `transform: translateY(-1px)`，而**祖先只要有 transform，
       后代元素的 `position: fixed` 就不再相对视口、而是相对那个祖先定位** ——
       菜单的位置会跟着卡片算错、跑到卡片外面去；鼠标一离开卡片 transform 消失，
       它又"跳"回正确位置。用户看到的就是"鼠标在菜单旁边时它不显示，移开才冒出来"。
       挂到 body 上就跟卡片的那点变换彻底无关了。 -->
  <Teleport to="body">
    <div v-if="menu" ref="menuEl" class="ctx" role="menu" :aria-label="menuLabel" :style="place">
      <button
        type="button"
        class="ctx-item"
        role="menuitem"
        :title="`${menuLabel}：${full}`"
        @click="pickFolder"
      >
        <span aria-hidden="true">📂</span>
        {{ menuLabel }}
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.art-cell {
  display: inline-flex;
  max-width: 100%;
}

.art {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  padding: 2px 7px;
  border: 1px solid var(--xd-border);
  border-radius: 6px;
  background: var(--xd-card-sub);
  color: var(--xd-text-sub);
  font-size: calc(13.2px * var(--xd-font-scale));
  transition: all 0.15s var(--xd-ease);
}

.art:hover {
  color: var(--xd-accent);
  border-color: var(--xd-accent);
  background: var(--xd-accent-soft);
}

.art.failed {
  color: var(--xd-red);
  border-color: var(--xd-red);
  background: transparent;
}

/* emoji 自带基线偏移，`line-height: 1` 压掉它，否则小签会被顶高一行 */
.art-icon {
  flex: none;
  font-size: calc(12px * var(--xd-font-scale));
  line-height: 1;
}

.art-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 菜单被 Teleport 到 body 上（躲开卡片的 transform），但样式是 scoped 的，
   Vue 会给它带上 data-v 属性，所以这条规则照样命中。 */
.ctx {
  position: fixed;
  z-index: 55;
  padding: 4px;
  border: 1px solid var(--xd-border);
  border-radius: 10px;
  background: var(--xd-card);
  box-shadow: var(--xd-shadow-pop);
}

.ctx-item {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 7px 9px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--xd-text);
  font-size: calc(13.2px * var(--xd-font-scale));
  text-align: left;
  cursor: pointer;
  transition:
    background 0.14s var(--xd-ease),
    color 0.14s var(--xd-ease);
}

.ctx-item:hover {
  background: var(--xd-accent-soft);
  color: var(--xd-accent);
}
</style>
