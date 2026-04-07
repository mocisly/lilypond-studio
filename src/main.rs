// 声明模块
mod app;        // 导入 app 模块
mod render;     // 导入 render 模块
mod scores;     // 导入 scores 模块
mod tutorial;   // 导入 tutorial 模块

// 导入标准库
use std::fs;                                // 文件系统操作
use std::path::PathBuf;                     // 路径处理
use std::time::{SystemTime, UNIX_EPOCH};    // 时间处理

// 主函数
fn main() {
    // 获取当前会话的渲染根目录
    let render_root = session_render_root();
    
    // 创建渲染工作目录（如果不存在则创建）
    fs::create_dir_all(&render_root)
        // 如果失败则panic并显示错误信息
        .expect("failed to create the render workspace");
    
    // 获取默认的数据库文件路径
    let database_path = scores::default_database_path();
    
    // 运行应用程序，传入渲染目录和数据库路径
    app::run(render_root, database_path);
}

// 生成会话特定的渲染根目录路径
fn session_render_root() -> PathBuf {
    // 获取当前系统时间
    let timestamp = SystemTime::now()
        // 计算自UNIX纪元以来的时间间隔
        .duration_since(UNIX_EPOCH)
        // 处理时间倒退的情况
        .expect("system clock should be after unix epoch")
        // 转换为毫秒
        .as_millis();

    // 生成临时目录路径，格式为：临时目录/lilypond-studio-进程ID-时间戳
    std::env::temp_dir().join(format!(
        // 文件名模板
        "lilypond-studio-{}-{timestamp}",
        // 当前进程ID
        std::process::id()
    ))
}
