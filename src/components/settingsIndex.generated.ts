// 此文件由 scripts/gen-settings-index.mjs 生成，请勿手改。
//
// 设置面板按大类懒加载，没点开的大类不在 DOM 里 —— 搜索只能查这份静态索引。
// 改完面板模板后跑 `npm run gen:settings-index`，或直接 `npm run build`（会自动跑）。

/** 一条设置项索引：它在哪个分区、标题是什么（分区到大类由 SettingsView 查表得到） */
export interface SettingsIndexEntry {
  section: string
  title: string
}

export const SETTINGS_INDEX: SettingsIndexEntry[] = [
  { section: 'shortcut', title: "全局快捷键" },
  { section: 'window', title: "关闭窗口时" },
  { section: 'window', title: "窗口置顶" },
  { section: 'window', title: "贴边自动隐藏" },
  { section: 'startup', title: "开机自动启动" },
  { section: 'notify', title: "到期提醒的通知" },
  { section: 'notify', title: "番茄钟的通知" },
  { section: 'theme', title: "主题模式" },
  { section: 'font', title: "界面字体" },
  { section: 'accent', title: "浅色模式" },
  { section: 'accent', title: "深色模式" },
  { section: 'background', title: "卡片不透明度" },
  { section: 'background', title: "卡片模糊度" },
  { section: 'background', title: "窗口圆角" },
  { section: 'background', title: "壁纸" },
  { section: 'background', title: "内置壁纸" },
  { section: 'background', title: "壁纸蒙版" },
  { section: 'background', title: "背景处理" },
  { section: 'background', title: "壁纸覆盖顶部" },
  { section: 'file', title: "待办文件" },
  { section: 'view', title: "默认视图" },
  { section: 'view', title: "显示已删除" },
  { section: 'domains', title: "当前分类" },
  { section: 'remind', title: "到期提醒" },
  { section: 'remind', title: "提前几天提醒" },
  { section: 'remind', title: "立即检查" },
  { section: 'focus', title: "专注时长" },
  { section: 'focus', title: "短休时长" },
  { section: 'focus', title: "长休时长" },
  { section: 'focus', title: "长休前的轮数" },
  { section: 'focus', title: "自动开始下一阶段" },
  { section: 'focus', title: "到点提示音" },
  { section: 'mode', title: "md / txt 用什么打开" },
  { section: 'mode', title: "MD-Preview 位置" },
  { section: 'roots', title: "允许打开的目录" },
  { section: 'storage', title: "数据路径" },
  { section: 'storage', title: "日志目录" },
  { section: 'backup', title: "每日备份" },
  { section: 'backup', title: "保留份数" },
  { section: 'backup', title: "立即备份" },
  { section: 'backup', title: "从备份恢复" },
  { section: 'danger', title: "初始化" },
  { section: 'about', title: "小刀工作台" },
  { section: 'about', title: "更新" },
  { section: 'changelog', title: "每个版本改了什么" },
  { section: 'studio', title: "官网" },
  { section: 'studio', title: "邮箱" },
  { section: 'privacy', title: "数据只在本机" },
  { section: 'privacy', title: "唯一的联网请求" },
]
