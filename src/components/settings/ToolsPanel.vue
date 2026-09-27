<script setup lang="ts">
// 工具：番茄闹钟 / GitHub 热榜 / 记账的设置，跟着侧边栏长 —— 侧边栏加工具，这里加分区。
//
// 时长类设置用**固定档位**而不是自由输入：防手滑填出 0 分钟或 999 分钟，
// 也免了后端再夹一遍。档位覆盖了市面番茄钟的常见配置。
import { inject } from 'vue'
import { configStore, saveConfig } from '../../stores/config'

const showToast = inject<(s: string) => void>('showToast', () => {})

const WORK_STEPS = [15, 20, 25, 30, 45, 60]
const SHORT_STEPS = [3, 5, 10]
const LONG_STEPS = [10, 15, 20, 30]
const ROUND_STEPS = [2, 3, 4, 5, 6]

async function setWork(v: number): Promise<void> {
  await saveConfig({ focus_work_min: v })
}
async function setShort(v: number): Promise<void> {
  await saveConfig({ focus_short_min: v })
}
async function setLong(v: number): Promise<void> {
  await saveConfig({ focus_long_min: v })
}
async function setRounds(v: number): Promise<void> {
  await saveConfig({ focus_rounds: v })
}
async function toggleAuto(): Promise<void> {
  await saveConfig({ focus_auto_continue: !configStore.cfg.focus_auto_continue })
}
async function toggleSound(): Promise<void> {
  await saveConfig({ focus_sound: !configStore.cfg.focus_sound })
}

/** 改完当前阶段的时长提示一句：正在跑的阶段不受影响，别让用户以为没生效 */
function changed(label: string): void {
  showToast(`${label}已保存（正在进行的阶段不受影响）`)
}
</script>

<template>
  <!-- 番茄闹钟 -->
  <section id="sv-sec-focus" class="sv-sec">
    <h3 class="sv-sec-title">番茄闹钟</h3>
    <p class="sv-sec-note">到点发一条系统通知。改完下一阶段生效，正在跑的不受影响。</p>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">专注时长</span>
        <span class="setting-desc">一个番茄多久</span>
      </div>
      <div class="sv-seg">
        <button
          v-for="v in WORK_STEPS"
          :key="v"
          type="button"
          class="sv-btn"
          :class="{ on: configStore.cfg.focus_work_min === v }"
          @click="setWork(v).then(() => changed('专注时长'))"
        >
          {{ v }} 分
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">短休时长</span>
        <span class="setting-desc">每个番茄之间的休息</span>
      </div>
      <div class="sv-seg">
        <button
          v-for="v in SHORT_STEPS"
          :key="v"
          type="button"
          class="sv-btn"
          :class="{ on: configStore.cfg.focus_short_min === v }"
          @click="setShort(v).then(() => changed('短休时长'))"
        >
          {{ v }} 分
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">长休时长</span>
        <span class="setting-desc">连续几个番茄后的大休息</span>
      </div>
      <div class="sv-seg">
        <button
          v-for="v in LONG_STEPS"
          :key="v"
          type="button"
          class="sv-btn"
          :class="{ on: configStore.cfg.focus_long_min === v }"
          @click="setLong(v).then(() => changed('长休时长'))"
        >
          {{ v }} 分
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">长休前的轮数</span>
        <span class="setting-desc">专注几轮后接一次长休</span>
      </div>
      <div class="sv-seg">
        <button
          v-for="v in ROUND_STEPS"
          :key="v"
          type="button"
          class="sv-btn"
          :class="{ on: configStore.cfg.focus_rounds === v }"
          @click="setRounds(v).then(() => changed('长休轮数'))"
        >
          {{ v }} 轮
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">自动开始下一阶段</span>
        <span class="setting-desc">
          专注完自动开始休息、休息完自动开始下一个番茄。默认关 ——
          每个番茄由你亲手按下开始，节奏自己说了算
        </span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.focus_auto_continue"
        :class="{ on: configStore.cfg.focus_auto_continue }"
        @click="toggleAuto"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">到点提示音</span>
        <span class="setting-desc">
          一声柔和的双音（系统通知始终会发，这里只管响不响）
        </span>
      </div>
      <button
        class="toggle"
        role="switch"
        type="button"
        :aria-checked="configStore.cfg.focus_sound"
        :class="{ on: configStore.cfg.focus_sound }"
        @click="toggleSound"
      >
        <span class="toggle-knob"></span>
      </button>
    </div>
  </section>
</template>
