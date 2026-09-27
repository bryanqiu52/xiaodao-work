import { computed, type ComputedRef } from 'vue'
import { configStore } from '../stores/config'
import { DOMAIN_LABELS, DOMAIN_OPTIONS } from '../core/constants'
import type { Domain } from '../core/types'

/**
 * 当前生效的领域分类 —— **响应式**。
 *
 * 为什么需要它：`core/constants` 里还有一份 `activeDomainOptions()`，
 * 但那份是给**读盘归一化**用的（模块变量，不触发渲染）。界面上的选项和标签必须
 * 跟着 `configStore` 走 —— 否则用户在设置里加完分类，回到待办页看到的还是老选项，
 * 得重启才生效，那跟"改了没用"没区别。
 */
export function useDomains(): {
  options: ComputedRef<Domain[]>
  label: (key: string) => string
} {
  /** 配置里配了就用配置的，没配用内置那 6 类 */
  const options = computed<Domain[]>(() =>
    // 自定义分类是运行时才有的值，天然不可能落在 `Domain` 这个联合类型里 ——
    // 这里的断言就是这个意思：分类现在是开放的，不再是那 6 个固定值
    configStore.cfg.domains.length > 0
      ? (configStore.cfg.domains as Domain[])
      : [...DOMAIN_OPTIONS],
  )

  /** 中文名：自定义 > 内置 > 原样显示 key（自定义分类没起名时也不会空着） */
  const label = (key: string): string =>
    configStore.cfg.domain_labels[key] ?? DOMAIN_LABELS[key] ?? key

  return { options, label }
}
