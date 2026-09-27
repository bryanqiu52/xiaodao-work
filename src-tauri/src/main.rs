// 发布版隐藏控制台窗口（调试版保留，便于看日志）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    xiaodao_work_lib::run()
}
