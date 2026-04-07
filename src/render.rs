// 导入依赖
use anyhow::{Context, Result, bail};    // 错误处理库
use std::ffi::OsStr;                    // 操作系统字符串
use std::fs;                            // 文件系统操作
use std::path::{Path, PathBuf};         // 路径处理
use std::process::Command;              // 外部命令执行

// 表示渲染后的页面
pub struct RenderedPage {
    pub path: PathBuf,     // SVG 文件路径
    pub width_px: f32,     // 页面宽度（像素）
    pub height_px: f32,    // 页面高度（像素）
}

// 渲染结果
pub struct RenderOutcome {
    pub pages: Vec<RenderedPage>,  // 渲染后的页面列表
    pub log: String,               // 渲染日志
}

// 渲染乐谱的主要函数
pub fn render_score(source: &str, render_root: &Path, job_id: u64) -> Result<RenderOutcome> {
    // 1. 创建渲染根目录
    fs::create_dir_all(render_root).context("failed to create the render root")?;
    
    // 2. 为每个渲染任务创建独立的目录
    let job_dir = render_root.join(format!("render-{job_id:04}"));
    fs::create_dir_all(&job_dir).with_context(|| {
        format!(
            "failed to create the job render directory at {}",
            job_dir.display()
        )
    })?;
    
    // 3. 将 LilyPond 源代码写入文件
    let source_path = job_dir.join("score.ly");
    fs::write(&source_path, source).with_context(|| {
        format!(
            "failed to write the LilyPond source file to {}",
            source_path.display()
        )
    })?;
    
    // 4. 设置输出文件前缀
    let output_prefix = job_dir.join("score");
    
    // 5. 执行 lilypond 命令
    let output = Command::new("lilypond")
        // 生成 SVG 格式
        .arg("--svg")
        // 禁用点选功能
        .arg("-dno-point-and-click")
        // 指定输出文件前缀
        .arg("-o")
        // 输出文件路径
        .arg(&output_prefix)
        // 源文件路径
        .arg(&source_path)
        // 执行命令并获取输出
        .output()
        // 如果执行失败
        .context("failed to execute the `lilypond` CLI from PATH")?;
    
    // 6. 合并标准输出和标准错误
    let log = combined_output(&output.stdout, &output.stderr);
    
    // 7. 检查命令是否成功执行
    if !output.status.success() {
        let detail = if log.is_empty() {
            format!("lilypond exited with status {}", output.status)
        } else {
            log
        };
        // 如果失败，返回错误
        bail!("{detail}");
    }
    
    // 8. 收集生成的 SVG 文件
    let mut pages = fs::read_dir(&job_dir)
        // 目录读取错误处理
        .with_context(|| format!("failed to inspect {}", job_dir.display()))?
        // 过滤有效条目
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        // 只保留 .svg 文件
        .filter(|path| path.extension() == Some(OsStr::new("svg")))
        .map(|path| {
            // 为每个 SVG 文件读取尺寸信息
            let (width_px, height_px) = read_svg_dimensions(&path)?;
            Ok(RenderedPage {
                path,
                width_px,
                height_px,
            })
        })
        // 收集结果，处理错误
        .collect::<Result<Vec<_>>>()?;
    
    // 9. 按路径排序页面
    pages.sort_by(|left, right| left.path.cmp(&right.path));
    
    // 10. 检查是否有页面生成
    if pages.is_empty() {
        bail!("lilypond completed, but no SVG pages were produced");
    }
    
    // 11. 返回渲染结果
    Ok(RenderOutcome { pages, log })
}

// 合并标准输出和标准错误
fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    // 将字节转换为字符串
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();

    // 根据内容情况返回合适的字符串
    match (stdout.is_empty(), stderr.is_empty()) {
        // 都没有内容
        (true, true) => String::new(),
        // 只有标准输出
        (false, true) => stdout,
        // 只有标准错误
        (true, false) => stderr,
        // 两者都有
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

// 从 SVG 文件中读取尺寸信息
fn read_svg_dimensions(path: &Path) -> Result<(f32, f32)> {
    // 1. 读取 SVG 文件内容
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read generated SVG at {}", path.display()))?;
    
    // 2. 查找 SVG 开始标签
    let svg_tag_end = contents
        .find('>')
        .context("generated SVG is missing its opening tag")?;
    // 获取 SVG 开始标签部分
    let svg_tag = &contents[..svg_tag_end];
    
    // 3. 提取宽度
    let width = extract_svg_dimension(svg_tag, "width")
        .or_else(|| extract_view_box_dimension(svg_tag, 2))
        .context("generated SVG is missing width metadata")?;
    
    // 4. 提取高度
    let height = extract_svg_dimension(svg_tag, "height")
        .or_else(|| extract_view_box_dimension(svg_tag, 3))
        .context("generated SVG is missing height metadata")?;
    
    Ok((width, height))
}

// 从 SVG 标签中提取特定维度属性
fn extract_svg_dimension(svg_tag: &str, attribute: &str) -> Option<f32> {
    // 构建要查找的属性字符串
    let needle = format!("{attribute}=\"");
    // 查找属性开始位置
    let start = svg_tag.find(&needle)? + needle.len();
    let rest = &svg_tag[start..];
    // 查找属性结束位置
    let end = rest.find('"')?;
    // 获取属性值
    let raw = &rest[..end];  
    
    // 将尺寸字符串转换为像素值
    parse_dimension_to_px(raw)
}

// 从 viewBox 属性中提取维度
fn extract_view_box_dimension(svg_tag: &str, index: usize) -> Option<f32> {
    // 查找 viewBox 属性
    let needle = "viewBox=\"";
    let start = svg_tag.find(needle)? + needle.len();
    let rest = &svg_tag[start..];
    let end = rest.find('"')?;
    
    // 解析 viewBox 值（格式：x y width height）
    rest[..end]
        // 按空白分割
        .split_whitespace()
        // 获取指定索引的值
        .nth(index)?
        // 转换为浮点数
        .parse::<f32>()
        // 转换失败返回 None
        .ok()
}

// 将尺寸字符串转换为像素值
fn parse_dimension_to_px(raw: &str) -> Option<f32> {
    // 去除首尾空格
    let raw = raw.trim();
    
    // 查找数值部分的结束位置
    let numeric_end = raw
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(raw.len());
    // 解析数值部分
    let value = raw[..numeric_end].parse::<f32>().ok()?;
    // 获取单位部分
    let unit = raw[numeric_end..].trim();
    
    // 根据单位转换到像素
    let pixels = match unit {
        // 无单位或像素
        "" | "px" => value,
        // 点（1pt = 1/72 inch）
        "pt" => value * (96.0 / 72.0),
        // 派卡（1pc = 12pt）
        "pc" => value * 16.0,
        // 毫米
        "mm" => value * (96.0 / 25.4),
        // 厘米
        "cm" => value * (96.0 / 2.54),
        // 英寸（1in = 96px）
        "in" => value * 96.0,
        // 不支持的单位
        _ => return None,
    };
    
    Some(pixels)
}
