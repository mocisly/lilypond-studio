use anyhow::{Context, Result, bail};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct RenderedPage {
    pub path: PathBuf,
    pub width_px: f32,
    pub height_px: f32,
}

pub struct RenderOutcome {
    pub pages: Vec<RenderedPage>,
    pub log: String,
}

pub fn render_score(source: &str, render_root: &Path, job_id: u64) -> Result<RenderOutcome> {
    fs::create_dir_all(render_root).context("failed to create the render root")?;

    let job_dir = render_root.join(format!("render-{job_id:04}"));
    fs::create_dir_all(&job_dir).with_context(|| {
        format!(
            "failed to create the job render directory at {}",
            job_dir.display()
        )
    })?;

    let source_path = job_dir.join("score.ly");
    fs::write(&source_path, source).with_context(|| {
        format!(
            "failed to write the LilyPond source file to {}",
            source_path.display()
        )
    })?;

    let output_prefix = job_dir.join("score");
    let output = Command::new("lilypond")
        .arg("--svg")
        .arg("-dno-point-and-click")
        .arg("-o")
        .arg(&output_prefix)
        .arg(&source_path)
        .output()
        .context("failed to execute the `lilypond` CLI from PATH")?;

    let log = combined_output(&output.stdout, &output.stderr);

    if !output.status.success() {
        let detail = if log.is_empty() {
            format!("lilypond exited with status {}", output.status)
        } else {
            log
        };
        bail!("{detail}");
    }

    let mut pages = fs::read_dir(&job_dir)
        .with_context(|| format!("failed to inspect {}", job_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension() == Some(OsStr::new("svg")))
        .map(|path| {
            let (width_px, height_px) = read_svg_dimensions(&path)?;
            Ok(RenderedPage {
                path,
                width_px,
                height_px,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    pages.sort_by(|left, right| left.path.cmp(&right.path));

    if pages.is_empty() {
        bail!("lilypond completed, but no SVG pages were produced");
    }

    Ok(RenderOutcome { pages, log })
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn read_svg_dimensions(path: &Path) -> Result<(f32, f32)> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read generated SVG at {}", path.display()))?;

    let svg_tag_end = contents
        .find('>')
        .context("generated SVG is missing its opening tag")?;
    let svg_tag = &contents[..svg_tag_end];

    let width = extract_svg_dimension(svg_tag, "width")
        .or_else(|| extract_view_box_dimension(svg_tag, 2))
        .context("generated SVG is missing width metadata")?;
    let height = extract_svg_dimension(svg_tag, "height")
        .or_else(|| extract_view_box_dimension(svg_tag, 3))
        .context("generated SVG is missing height metadata")?;

    Ok((width, height))
}

fn extract_svg_dimension(svg_tag: &str, attribute: &str) -> Option<f32> {
    let needle = format!("{attribute}=\"");
    let start = svg_tag.find(&needle)? + needle.len();
    let rest = &svg_tag[start..];
    let end = rest.find('"')?;
    let raw = &rest[..end];

    parse_dimension_to_px(raw)
}

fn extract_view_box_dimension(svg_tag: &str, index: usize) -> Option<f32> {
    let needle = "viewBox=\"";
    let start = svg_tag.find(needle)? + needle.len();
    let rest = &svg_tag[start..];
    let end = rest.find('"')?;
    rest[..end]
        .split_whitespace()
        .nth(index)?
        .parse::<f32>()
        .ok()
}

fn parse_dimension_to_px(raw: &str) -> Option<f32> {
    let raw = raw.trim();
    let numeric_end = raw
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(raw.len());
    let value = raw[..numeric_end].parse::<f32>().ok()?;
    let unit = raw[numeric_end..].trim();

    let pixels = match unit {
        "" | "px" => value,
        "pt" => value * (96.0 / 72.0),
        "pc" => value * 16.0,
        "mm" => value * (96.0 / 25.4),
        "cm" => value * (96.0 / 2.54),
        "in" => value * 96.0,
        _ => return None,
    };

    Some(pixels)
}
