// 导入依赖
use crate::render::{render_score, RenderOutcome, RenderedPage};                             // 渲染模块
use crate::scores::{ScoreManager, SqliteScoreStore};                                        // 乐谱管理模块
use crate::tutorial::LESSONS;                                                               // 教程模块
use gpui::prelude::*;                                                                       // GPUI 前置导入
use gpui::{
    AnyElement, App, Application, Bounds, Context, Entity, FontWeight, KeyBinding, Menu, MenuItem,
    SharedString, Subscription, Window, WindowBounds, WindowOptions, actions, div, img, px, size,
};                                                                                          // GPUI 核心类型和功能
use gpui_component::button::{Button, ButtonVariants as _};                                  // 按钮组件
use gpui_component::input::{Input, InputEvent, InputState};                                 // 输入组件
use gpui_component::scroll::ScrollableElement as _;                                         // 滚动组件
use gpui_component::{ActiveTheme, Disableable, Icon, IconName, Root, Sizable, TitleBar};    // 组件库
use std::path::PathBuf;                                                                     // 路径处理
use std::sync::Arc;                                                                         // 原子引用计数

// 定义应用程序动作
actions!(studio, [Quit]);  // 定义退出动作

// 主运行函数
pub fn run(render_root: PathBuf, database_path: PathBuf) {
    // 创建并运行应用程序
    Application::new().run(move |cx: &mut App| {
        // 初始化 GPUI 组件
        gpui_component::init(cx);

        // 设置退出动作处理器
        cx.on_action(|_: &Quit, cx| cx.quit());

        // 绑定键盘快捷键
        cx.bind_keys([
            // macOS 快捷键
            KeyBinding::new("cmd-q", Quit, None),
            // Windows/Linux 快捷键
            KeyBinding::new("ctrl-q", Quit, None),
        ]);

        // 设置应用程序菜单
        cx.set_menus(vec![Menu {
            // 菜单名称
            name: "LilyPond Studio".into(),
            // 菜单项
            items: vec![MenuItem::action("Quit", Quit)],
        }]);

        // 根据不同操作系统设置窗口大小
        #[cfg(target_os = "windows")]
        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        #[cfg(target_os = "macos")]
        let bounds = Bounds::centered(None, size(px(1440.0), px(940.0)), cx);
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let bounds = Bounds::centered(None, size(px(1360.0), px(900.0)), cx);

        // 设置窗口选项（Linux 和其他系统略有不同）
        #[cfg(target_os = "linux")]
        let window_options = WindowOptions {
            // 窗口边界
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // 标题栏
            titlebar: Some(TitleBar::title_bar_options()),
            // 窗口装饰
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..Default::default()
        };
        #[cfg(not(target_os = "linux"))]
        let window_options = WindowOptions {
            // 窗口边界
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // 标题栏
            titlebar: Some(TitleBar::title_bar_options()),
            ..Default::default()
        };

        // 打开应用程序窗口
        cx.open_window(window_options, {
            let render_root = render_root.clone();
            let database_path = database_path.clone();
            move |window, cx| {
                // 创建 StudioApp 实例
                let studio = cx.new(|cx| {
                    StudioApp::new(render_root.clone(), database_path.clone(), window, cx)
                });

                // 开始渲染
                let _ = studio.update(cx, |studio, cx| studio.begin_render(cx));

                // 创建根组件
                cx.new(|cx| Root::new(studio, window, cx))
            }
        })
        .expect("failed to open the LilyPond Studio window");

        // 激活应用程序
        cx.activate(true);
    });
}

// 渲染阶段枚举
#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderPhase {
    Idle,               // 空闲状态
    Rendering,          // 正在渲染
    Ready,              // 渲染完成
    Error,              // 渲染错误
}

// 应用程序屏幕枚举
#[derive(Clone, Copy, PartialEq, Eq)]
enum AppScreen {
    Studio,             // 工作室屏幕（编辑和预览）
    Scores,             // 乐谱库屏幕
}

// 活动文档枚举
#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveDocument {
    Score(i64),         // 乐谱（带有ID）
    Tutorial(usize),    // 教程（带有索引）
}

// 应用程序主结构体
struct StudioApp {
    editor: Entity<InputState>,             // 编辑器组件实体
    score_title: Entity<InputState>,        // 乐谱标题输入框实体
    score_manager: ScoreManager,            // 乐谱管理器
    current_screen: AppScreen,              // 当前屏幕
    active_document: ActiveDocument,        // 活动文档
    render_phase: RenderPhase,              // 渲染阶段
    render_root: PathBuf,                   // 渲染根目录
    next_render_job: u64,                   // 下一个渲染任务ID
    active_render_job: Option<u64>,         // 当前活动的渲染任务ID
    preview_pages: Vec<RenderedPage>,       // 预览页面列表
    current_preview_page: usize,            // 当前预览页面索引
    preview_zoom: f32,                      // 预览缩放级别
    render_log: SharedString,               // 渲染日志
    dirty: bool,                            // 标记是否需要重新渲染
    suppress_input_sync: bool,              // 是否禁止输入同步
    _subscriptions: Vec<Subscription>,      // 事件订阅（用于清理）
}

impl StudioApp {
    const DEFAULT_PREVIEW_ZOOM: f32 = 1.0;  // 默认缩放级别
    const MIN_PREVIEW_ZOOM: f32 = 0.5;      // 最小缩放级别
    const MAX_PREVIEW_ZOOM: f32 = 3.0;      // 最大缩放级别
    const PREVIEW_ZOOM_STEP: f32 = 0.25;    // 缩放步长

    // 构造函数
    fn new(
        render_root: PathBuf,
        database_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // 打开乐谱数据库
        let store = Arc::new(
            SqliteScoreStore::open(&database_path).expect("failed to open the score database"),
        );

        // 加载乐谱管理器
        let score_manager =
            ScoreManager::load(store).expect("failed to initialize the score manager");

        // 获取初始选中的乐谱
        let initial_score = score_manager.selected_score().clone();

        // 创建编辑器组件
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                // 设置为代码编辑器模式
                .code_editor("lilypond")
                // 显示行号
                .line_number(true)
                // 设置行数
                .rows(28)
                // 禁用软换行
                .soft_wrap(false)
                // 占位符文本
                .placeholder("Write LilyPond notation here and render it to SVG.")
                // 默认值
                .default_value(initial_score.source.clone())
        });

        // 创建乐谱标题输入框
        let score_title = cx.new(|cx| {
            InputState::new(window, cx)
                // 占位符文本
                .placeholder("Score title")
                // 默认值
                .default_value(initial_score.title.clone())
        });

        // 设置事件订阅
        let subscriptions = vec![
            // 编辑器内容变化订阅
            cx.subscribe_in(&editor, window, |this, _, event: &InputEvent, _, cx| {
                // 只有在内容变化且不禁用同步时才处理
                if !matches!(event, InputEvent::Change) || this.suppress_input_sync {
                    return;
                }

                // 根据活动文档类型处理
                if matches!(this.active_document, ActiveDocument::Score(_)) {
                    // 如果是乐谱，更新源码
                    let source = this.editor.read(cx).value().to_string();
                    if let Err(err) = this.score_manager.update_selected_source(source) {
                        this.render_phase = RenderPhase::Error;
                        this.render_log = format!("{err:#}").into();
                    } else {
                        // 标记预览为过时
                        this.mark_preview_stale();
                    }
                } else {
                    // 如果是教程，只标记预览为过时
                    this.mark_preview_stale();
                }
                // 通知UI更新
                cx.notify();
            }),

            // 标题输入框变化订阅
            cx.subscribe_in(
                &score_title,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    // 只有在内容变化且不禁用同步时才处理
                    if !matches!(event, InputEvent::Change) || this.suppress_input_sync {
                        return;
                    }

                    // 如果是乐谱文档，重命名选中的乐谱
                    let title = this.score_title.read(cx).value().to_string();
                    if let ActiveDocument::Score(_) = this.active_document {
                        if let Err(err) = this.score_manager.rename_selected_score(title) {
                            this.render_phase = RenderPhase::Error;
                            this.render_log = format!("{err:#}").into();
                        }
                    }
                    // 通知UI更新
                    cx.notify();
                },
            ),
            ];

        // 返回应用程序实例
        Self {
            editor,
            score_title,
            score_manager,
            // 初始显示乐谱库屏幕
            current_screen: AppScreen::Scores,
            // 初始活动文档
            active_document: ActiveDocument::Score(initial_score.id),
            // 初始渲染状态
            render_phase: RenderPhase::Idle,
            render_root,
            // 下一个渲染任务ID从0开始
            next_render_job: 0,
            // 没有活动的渲染任务
            active_render_job: None,
            // 空的预览页面列表
            preview_pages: Vec::new(),
            // 初始预览页面
            current_preview_page: 0,
            // 默认缩放级别
            preview_zoom: Self::DEFAULT_PREVIEW_ZOOM,
            // 空的渲染日志
            render_log: SharedString::default(),
            // 初始时标记为过时
            dirty: true,
            // 不禁用输入同步
            suppress_input_sync: false,
            // 保存订阅
            _subscriptions: subscriptions,
        }
    }

    // 获取文件数量摘要
    fn file_count_summary(&self) -> String {
        match self.score_manager.scores().len() + LESSONS.len() {
            1 => "1 file".to_string(),
            count => format!("{count} files"),
        }
    }

    // 获取教程数量摘要
    fn tutorial_count_summary(&self) -> String {
        match LESSONS.len() {
            1 => "1 tutorial".to_string(),
            count => format!("{count} tutorials"),
        }
    }

    // 获取选中文档的标题
    fn selected_document_title(&self) -> &str {
        match self.active_document {
            // 乐谱标题
            ActiveDocument::Score(_) => &self.score_manager.selected_score().title,
            // 教程标题
            ActiveDocument::Tutorial(index) => LESSONS[index].title,
        }
    }

    // 检查是否教程是活动的
    fn is_tutorial_active(&self) -> bool {
        matches!(self.active_document, ActiveDocument::Tutorial(_))
    }

    // 显示工作室屏幕
    fn show_studio_screen(&mut self, cx: &mut Context<Self>) {
        if self.current_screen != AppScreen::Studio {
            // 切换屏幕
            self.current_screen = AppScreen::Studio;
            // 通知UI更新
            cx.notify();
        }
    }

    // 显示乐谱库屏幕
    fn show_scores_screen(&mut self, cx: &mut Context<Self>) {
        if self.current_screen != AppScreen::Scores {
            // 切换屏幕
            self.current_screen = AppScreen::Scores;
            // 通知UI更新
            cx.notify();
        }
    }

    // 开始渲染
    fn begin_render(&mut self, cx: &mut Context<Self>) {
        // 获取编辑器内容
        let source = self.editor.read(cx).value().to_string();
        // 使用下一个渲染任务ID
        let job_id = self.next_render_job;
        // 克隆渲染根目录
        let render_root = self.render_root.clone();

        // 递增任务ID
        self.next_render_job += 1;
        // 设置活动任务
        self.active_render_job = Some(job_id);
        // 设置为渲染中状态
        self.render_phase = RenderPhase::Rendering;
        // 清空日志
        self.render_log = SharedString::default();
        // 标记为不再过时
        self.dirty = false;
        // 通知UI更新
        cx.notify();

        // 在后台执行渲染任务
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { render_score(&source, &render_root, job_id) })
                .await;

            // 渲染完成后更新UI
            let _ = entity.update(cx, |studio, cx| studio.finish_render(job_id, result, cx));
        })
        // 分离任务，不等待完成
        .detach();
    }

    // 完成渲染
    fn finish_render(
        &mut self,
        job_id: u64,
        result: anyhow::Result<RenderOutcome>,
        cx: &mut Context<Self>,
    ) {
        // 如果不是当前的任务，忽略
        if self.active_render_job != Some(job_id) {
            return;
        }

        // 清除活动任务
        self.active_render_job = None;

        // 处理渲染结果
        match result {
            // 渲染成功
            Ok(outcome) => {
                // 保存预览页面
                self.preview_pages = outcome.pages;
                self.current_preview_page = 0;  // 重置到第一页
                self.render_phase = RenderPhase::Ready;  // 设置为就绪状态
                self.render_log = outcome.log.into();  // 保存渲染日志
            }
            // 渲染失败
            Err(err) => {
                // 设置为错误状态
                self.render_phase = RenderPhase::Error;
                // 保存错误信息
                self.render_log = format!("{err:#}").into();
            }
        }

        // 通知UI更新
        cx.notify();
    }

    // 标记预览为过时
    fn mark_preview_stale(&mut self) {
        self.dirty = true;
    }

    // 切换乐谱时重置预览
    fn reset_preview_for_score_switch(&mut self) {
        // 清除活动任务
        self.active_render_job = None;
        // 清空预览页面
        self.preview_pages.clear();
        // 重置到第一页
        self.current_preview_page = 0;
        // 重置缩放级别
        self.preview_zoom = Self::DEFAULT_PREVIEW_ZOOM;
        // 清空日志
        self.render_log = SharedString::default();
        // 设置为空闲状态
        self.render_phase = RenderPhase::Idle;
        // 标记为过时
        self.dirty = true;
    }

    // 同步选中的乐谱输入框
    fn sync_selected_score_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 获取选中的乐谱
        let score = self.score_manager.selected_score().clone();
        // 临时禁止输入同步
        self.suppress_input_sync = true;

        // 更新标题输入框
        let _ = self
            .score_title
            .update(cx, |input, cx| input.set_value(score.title, window, cx));

        // 更新编辑器内容
        let _ = self
            .editor
            .update(cx, |input, cx| input.set_value(score.source, window, cx));

        // 恢复输入同步
        self.suppress_input_sync = false;
    }

    // 选择乐谱
    fn select_score(&mut self, score_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        // 切换到指定乐谱
        if self.score_manager.select_score(score_id) {
            // 设置活动文档
            self.active_document = ActiveDocument::Score(score_id);
            // 同步输入框
            self.sync_selected_score_inputs(window, cx);
            // 重置预览
            self.reset_preview_for_score_switch();
            // 切换到工作室屏幕
            self.current_screen = AppScreen::Studio;
            // 通知UI更新
            cx.notify();
        }
    }

    // 打开教程
    fn open_tutorial(
        &mut self,
        tutorial_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 获取教程
        let lesson = &LESSONS[tutorial_index];
        // 设置活动文档为教程
        self.active_document = ActiveDocument::Tutorial(tutorial_index);
        // 临时禁止输入同步
        self.suppress_input_sync = true;

        // 更新标题输入框
        let _ = self
            .score_title
            .update(cx, |input, cx| input.set_value(lesson.title, window, cx));

        // 更新编辑器内容
        let _ = self
            .editor
            .update(cx, |input, cx| input.set_value(lesson.example, window, cx));

        // 恢复输入同步
        self.suppress_input_sync = false;
        // 重置预览
        self.reset_preview_for_score_switch();
        // 切换到工作室屏幕
        self.current_screen = AppScreen::Studio;
        // 通知UI更新
        cx.notify();
    }

    // 创建新乐谱
    fn create_score(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 尝试创建乐谱
        if let Err(err) = self.score_manager.create_score() {
            // 显示错误
            self.render_phase = RenderPhase::Error;
            self.render_log = format!("{err:#}").into();
        } else {
            // 设置为活动文档
            self.active_document = ActiveDocument::Score(self.score_manager.selected_score_id());
            // 同步输入框
            self.sync_selected_score_inputs(window, cx);
            // 重置预览
            self.reset_preview_for_score_switch();
        }
        // 通知UI更新
        cx.notify();
    }

    // 删除选中的乐谱
    fn delete_selected_score(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 只有在乐谱文档时才允许删除
        if !matches!(self.active_document, ActiveDocument::Score(_)) {
            return;
        }

        // 尝试删除乐谱
        if let Err(err) = self.score_manager.delete_selected_score() {
            // 显示错误
            self.render_phase = RenderPhase::Error;
            self.render_log = format!("{err:#}").into();
        } else {
            // 更新活动文档
            self.active_document = ActiveDocument::Score(self.score_manager.selected_score_id());
            // 同步输入框
            self.sync_selected_score_inputs(window, cx);
            // 重置预览
            self.reset_preview_for_score_switch();
        }
        // 通知UI更新
        cx.notify();
    }

    // 上一预览页面
    fn previous_preview_page(&mut self, cx: &mut Context<Self>) {
        if self.current_preview_page > 0 {
            // 递减页面索引
            self.current_preview_page -= 1;
            // 通知UI更新
            cx.notify();
        }
    }

    // 下一预览页面
    fn next_preview_page(&mut self, cx: &mut Context<Self>) {
        if self.current_preview_page + 1 < self.preview_pages.len() {
            // 递增页面索引
            self.current_preview_page += 1;
            // 通知UI更新
            cx.notify();
        }
    }

    // 缩小预览
    fn zoom_out_preview(&mut self, cx: &mut Context<Self>) {
        // 缩小，但不小于最小值
        self.preview_zoom =
            (self.preview_zoom - Self::PREVIEW_ZOOM_STEP).max(Self::MIN_PREVIEW_ZOOM);
        // 通知UI更新
        cx.notify();
    }

    // 放大预览
    fn zoom_in_preview(&mut self, cx: &mut Context<Self>) {
        // 放大，但不超过最大值
        self.preview_zoom =
            (self.preview_zoom + Self::PREVIEW_ZOOM_STEP).min(Self::MAX_PREVIEW_ZOOM);
        // 通知UI更新
        cx.notify();
    }

    // 重置预览缩放
    fn reset_preview_zoom(&mut self, cx: &mut Context<Self>) {
        // 重置为默认缩放
        self.preview_zoom = Self::DEFAULT_PREVIEW_ZOOM;
        // 通知UI更新
        cx.notify();
    }

    // 获取预览缩放标签
    fn preview_zoom_label(&self) -> String {
        // 转换为百分比
        format!("{:.0}%", self.preview_zoom * 100.0)
    }

    // 获取预览页面摘要
    fn preview_page_summary(&self) -> String {
        match self.preview_pages.len() {
            // 没有预览
            0 => "No preview".to_string(),
            // 1页
            1 => "1 page".to_string(),
            // 多页
            count => format!("{count} pages"),
        }
    }

    // 创建头部芯片组件
    fn header_chip(&self, icon: IconName, label: impl Into<SharedString>, cx: &App) -> AnyElement {
        let label: SharedString = label.into();

        div()
            .flex()
            .items_center()
            .gap_1()
            .h(px(22.0))
            .px_2()
            // 背景色
            .bg(cx.theme().secondary)
            .border_1()
            // 边框色
            .border_color(cx.theme().border)
            // 圆角
            .rounded(px(999.0))
            .child(
                // 图标
                Icon::new(icon)
                .small()
                .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                // 文字大小
                .text_size(px(11.0))
                // 文字颜色
                .text_color(cx.theme().muted_foreground)
                // 标签
                .child(label),
            )
            // 转换为任意元素
            .into_any_element()
    }

    // 创建屏幕导航按钮
    fn screen_nav_button(&self, id: &'static str, label: &'static str, active: bool) -> Button {
        // 创建按钮
        let button = Button::new(id).label(label);
        // 根据是否活动设置样式
        if active { button.primary() } else { button }
    }

    // 创建应用程序头部
    fn app_header(&self, view: Entity<Self>, render_button: Button, cx: &App) -> AnyElement {
        // 工作室按钮
        let studio_button = self
            .screen_nav_button(
                "show-studio-screen",
                "Studio",
                self.current_screen == AppScreen::Studio,
            )
            .on_click({
                // 点击切换到工作室
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.show_studio_screen(cx));
                }
            });

        // 乐谱库按钮
        let scores_button = self
            .screen_nav_button(
                "show-scores-screen",
                "Scores",
                self.current_screen == AppScreen::Scores,
            )
            .on_click({
                // 点击切换到乐谱库
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.show_scores_screen(cx));
                }
            });

        // 创建标题栏
        TitleBar::new()
            .child(
                // 左侧：应用图标和标题
                div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    // 应用图标
                    div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(22.0))
                    .h(px(22.0))
                    .rounded(px(7.0))
                    // 主要颜色背景
                    .bg(cx.theme().primary)
                    // 主要颜色文字
                    .text_color(cx.theme().primary_foreground)
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    // 应用缩写
                    .child("LP"),
                )
                .child(
                    // 应用标题和当前文档标题
                    div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        // 应用名称
                        div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("LilyPond Studio"),
                    )
                    .child(
                        // 当前文档标签
                        div()
                        .h(px(20.0))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(999.0))
                        // 次要颜色背景
                        .bg(cx.theme().secondary)
                        .text_size(px(11.0))
                        // 柔和文字颜色
                        .text_color(cx.theme().muted_foreground)
                        // 当前文档标题
                        .child(self.selected_document_title().to_string()),
                    ),
        ),
        )
            .child(
                // 右侧：状态信息和操作按钮
                div()
                .flex()
                .items_center()
                .gap_2()
                .pr(px(12.0))
                // 文件数量
                .child(self.header_chip(IconName::File, self.file_count_summary(), cx))
                // 教程数量
                .child(self.header_chip(
                        IconName::BookOpen,
                        self.tutorial_count_summary(),
                        cx,
                ))
                // 预览页面
                .child(self.header_chip(IconName::File, self.preview_page_summary(), cx))
                // 工作室按钮
                .child(studio_button.compact())
                // 乐谱库按钮
                .child(scores_button.compact())
                // 渲染状态徽章
                .child(self.render_badge(cx))
                // 渲染按钮
                .child(render_button.compact()),
            )
            // 转换为任意元素
            .into_any_element()
    }

    // 创建渲染状态徽章
    fn render_badge(&self, cx: &App) -> AnyElement {
        // 根据渲染状态设置标签和颜色
        let (label, bg, fg) = match self.render_phase {
            RenderPhase::Idle => (
                "Ready to render",
                // 柔和背景
                cx.theme().muted,
                // 柔和文字
                cx.theme().muted_foreground,
            ),
            // 信息色
            RenderPhase::Rendering => ("Rendering", cx.theme().info, cx.theme().info_foreground),
            RenderPhase::Ready if self.dirty => (
                "Preview stale",
                // 警告色
                cx.theme().warning,
                // 警告文字
                cx.theme().warning_foreground,
            ),
            RenderPhase::Ready => (
                "Preview current",
                // 成功色
                cx.theme().success,
                // 成功文字
                cx.theme().success_foreground,
            ),
            RenderPhase::Error => (
                "Render failed",
                // 危险色
                cx.theme().danger,
                // 危险文字
                cx.theme().danger_foreground,
            ),
        };

        // 创建徽章元素
        div()
            .px_2()
            .py_1()
            // 圆角
            .rounded(px(999.0))
            // 背景色
            .bg(bg)
            // 文字颜色
            .text_color(fg)
            // 文字大小
            .text_size(px(12.0))
            // 字体粗细
            .font_weight(FontWeight::MEDIUM)
            // 标签文字
            .child(label)
            // 转换为任意元素
            .into_any_element()
    }

    // 获取渲染详情
    fn render_detail(&self) -> String {
        match self.render_phase {
            // 空闲状态提示
            RenderPhase::Idle => {
                "Edit the score in the center pane, then compile it with the LilyPond CLI."
                    .to_string()
            }
            // 渲染中提示
            RenderPhase::Rendering => {
                "Running LilyPond and collecting the generated SVG pages.".to_string()
            }
            // 预览过时提示
            RenderPhase::Ready if self.dirty => format!(
                "{} page(s) rendered. The current score changed since the last successful render.",
                self.preview_pages.len()
            ),
            // 预览就绪提示
            RenderPhase::Ready => format!(
                "{} page(s) rendered from the current editor content.",
                self.preview_pages.len()
            ),
            // 错误提示
            RenderPhase::Error => {
                "LilyPond returned an error. Fix the notation and render again.".to_string()
            }
        }
    }

    // 创建乐谱库屏幕
    fn score_library_screen(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // 获取当前视图实体
        let view = cx.entity();
        // 选中的乐谱ID
        let selected_score_id = self.score_manager.selected_score_id();
        // 是否可以删除且乐谱列表不为空
        let can_delete = matches!(self.active_document, ActiveDocument::Score(_))
            && !self.score_manager.scores().is_empty();  

        // 创建乐谱列表
        let score_list = self.score_manager.scores().iter().fold(
            // 垂直排列，间距2
            div().flex().flex_col().gap_2(),
            |list, score| {
                // 检查是否选中
                let is_selected =
                    matches!(self.active_document, ActiveDocument::Score(id) if id == score.id);

                // 创建乐谱按钮
                let mut button =
                    Button::new(("score-item", score.id as u64)).label(score.title.clone());
                // 选中的按钮使用主要样式
                if is_selected {
                    button = button.primary();
                }

                // 添加点击事件
                list.child(button.on_click({
                    // 点击选择乐谱
                    let view = view.clone();
                    let score_id = score.id;
                    move |_, window, cx| {
                        let _ =
                            view.update(cx, |studio, cx| studio.select_score(score_id, window, cx));
                    }
                }))
            },
            );

        // 创建教程列表
        let tutorial_list =
            LESSONS
            .iter()
            .enumerate()
            .fold(div().flex().flex_col().gap_2(), |list, (index, lesson)| {
                // 检查是否选中
                let is_selected = matches!(
                    self.active_document,
                    ActiveDocument::Tutorial(active) if active == index
                );

                // 创建教程按钮
                let mut button = Button::new(("tutorial-item", index as u64))
                    .label(lesson.title)
                    // 教程图标
                    .icon(IconName::BookOpen);
                if is_selected {
                    // 选中的按钮使用主要样式
                    button = button.primary();
                }

                // 添加点击事件
                list.child(button.on_click({
                    // 点击打开教程
                    let view = view.clone();
                    move |_, window, cx| {
                        let _ = view.update(cx, |studio, cx| {
                            studio.open_tutorial(index, window, cx)
                        });
                    }
                }))
            });

        // 创建按钮
        let create_button = Button::new("create-score").label("Create").on_click({
            // 点击创建乐谱
            let view = view.clone();
            move |_, window, cx| {
                let _ = view.update(cx, |studio, cx| studio.create_score(window, cx));
                }
        });

        // 删除按钮
        let delete_button = Button::new("delete-score")
            .label("Delete")
            // 根据条件禁用
            .disabled(!can_delete)
            .on_click({
                // 点击删除乐谱
                let view = view.clone();
                move |_, window, cx| {
                    let _ = view.update(cx, |studio, cx| studio.delete_selected_score(window, cx));
                }
            });

        // 构建乐谱库屏幕布局
        div()
            .flex()
            // 填充可用空间
            .flex_1()
            .min_h(px(0.0))
            .gap_4()
            .child(
                // 左侧面板
                div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                // 次要背景色
                .bg(cx.theme().secondary)
                .border_1()
                // 边框颜色
                .border_color(cx.theme().border)
                // 标题区域
                .child(
                    div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    // 主要标题
                    .child(
                        div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Score Library"),
                    )
                    // 说明文字
                    .child(
                        div()
                        .text_size(px(13.0))
                        // 柔和文字颜色
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "Browse the local score library on its own screen, switch between pieces, and keep the editor bound to the selected score.",
                        ),
                    ),
        )
            // 标题输入区域
            .child(
                div()
                .flex()
                .items_end()
                .gap_3()
                // 标题输入框容器
                .child(
                    div()
                    .flex_1()
                    .flex_col()
                    .gap_1()
                    // 标签
                    .child(
                        div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("File Name"),
                    )
                    // 标题输入框
                    .child(
                        // 如果是教程则禁用
                        Input::new(&self.score_title)
                        .disabled(self.is_tutorial_active()),
                    ),
                )
                // 创建按钮
                .child(create_button)
                // 删除按钮
                .child(delete_button),
                )
                    // 乐谱列表区域
                    .child(
                        div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        // 乐谱列表标题
                        .child(
                            div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Scores"),
                        )
                        // 乐谱列表容器（可滚动）
                        .child(
                            div()
                            .flex_1()
                            .min_h(px(0.0))
                            // 垂直滚动
                            .overflow_y_scrollbar()
                            // 乐谱列表
                            .child(score_list),
                        ),
                )
                    // 教程列表区域
                    .child(
                        div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        // 教程列表标题
                        .child(
                            div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Tutorial Files"),
                        )
                        // 教程列表
                        .child(tutorial_list),
                    )
                    // 信息提示区域
                    .child(
                        div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .p_3()
                        // 柔和背景
                        .bg(cx.theme().muted)
                        // 圆角
                        .rounded(px(12.0))
                        // 信息图标
                        .child(
                            Icon::new(IconName::Info)
                            .small()
                            // 柔和图标颜色
                            .text_color(cx.theme().muted_foreground),
                        )
                        // 提示文字
                        .child(
                            div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.is_tutorial_active() {
                                "Tutorial files open directly in the editor and can be rendered immediately. Rename and delete only apply to saved scores.".to_string()
                            } else {
                                format!(
                                    "Current score: \"{}\" (id {}).",
                                    self.selected_document_title(),
                                    selected_score_id
                                )
                            }),
                        ),
                ),
                )
                    // 转换为任意元素
                    .into_any_element()
    }

    // 创建工作室屏幕
    fn studio_screen(
        &self,
        cx: &mut Context<Self>,
        previous_page: Button,
        next_page: Button,
        zoom_out: Button,
        reset_zoom: Button,
        zoom_in: Button,
        preview_meta: String,
    ) -> AnyElement {
        // 获取主题
        let theme = cx.theme();

        div()
            .flex()
            .flex_1()
            .min_h(px(0.0))
            .gap_4()
            .p_4()
            // 左侧编辑器区域
            .child(
                div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                // 次要背景色
                .bg(theme.secondary)
                .border_1()
                // 边框颜色
                .border_color(theme.border)
                // 编辑器标题区域
                .child(
                    div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    // 编辑器标题
                    .child(
                        div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Score Editor"),
                    )
                    // 编辑器说明
                    .child(
                        div()
                        .text_size(px(13.0))
                        // 柔和文字颜色
                        .text_color(theme.muted_foreground)
                        .child(
                            "The editor is a GPUI-native multiline input bound to the selected score. `Cmd/Ctrl+F` opens in-editor search, and each render writes a fresh LilyPond job workspace.",
                        ),
                    ),
        )
            // 编辑器容器
            .child(
                div()
                .flex_1()
                .min_h(px(0.0))
                // 编辑器输入组件
                .child(Input::new(&self.editor).h_full()),
            ),
        )
            // 右侧预览区域
            .child(
                div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_4()
                // 预览面板
                .child(
                    div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .gap_3()
                    .p_4()
                    // 次要背景色
                    .bg(theme.secondary)
                    .border_1()
                    // 边框颜色
                    .border_color(theme.border)
                    // 预览标题和操作区域
                    .child(
                        div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_3()
                        // 标题区域
                        .child(
                            div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            // 预览标题
                            .child(
                                div()
                                .text_size(px(16.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("SVG Preview"),
                            )
                            // 渲染详情
                            .child(
                                div()
                                .text_size(px(13.0))
                                .text_color(theme.muted_foreground)
                                .child(self.render_detail()),
                            ),
                        )
                            // 操作按钮区域
                            .child(
                                div()
                                .flex()
                                .items_center()
                                .gap_2()
                                // 缩小按钮
                                .child(zoom_out)
                                // 重置缩放按钮
                                .child(reset_zoom)
                                // 放大按钮
                                .child(zoom_in)
                                // 上一页按钮
                                .child(previous_page)
                                // 页面信息
                                .child(
                                    // 页面信息文本
                                    div()
                                    .text_size(px(12.0))
                                    .text_color(theme.muted_foreground)
                                    .child(preview_meta),
                                )
                                // 下一页按钮
                                .child(next_page),
                            ),
        )
            // 预览区域
            .child(self.preview_surface(cx))
            .when(!self.render_log.is_empty(), |this| {
                // 如果有日志，显示日志面板
                this.child(self.render_log_panel(cx))
            }),
        ),
        )
            // 转换为任意元素
            .into_any_element()
    }

    // 创建预览区域
    fn preview_surface(&self, cx: &App) -> AnyElement {
        // 如果有预览页面
        if let Some(page) = self.preview_pages.get(self.current_preview_page) {
            // 计算缩放后的宽度
            let zoomed_width = page.width_px * self.preview_zoom;
            // 计算缩放后的高度
            let zoomed_height = page.height_px * self.preview_zoom;

            // 创建包含SVG的预览区域
            return div()
                .flex_1()
                .min_h(px(0.0))
                // 白色背景
                .bg(gpui::white())
                .border_1()
                // 边框颜色
                .border_color(cx.theme().border)
                // 滚动容器
                .child(
                    // 预览内容容器
                    div().size_full().overflow_scrollbar().child(
                        div()
                        .p_3()
                        // 宽度加上内边距
                        .w(px(zoomed_width + 24.0))
                        // 高度加上内边距
                        .h(px(zoomed_height + 24.0))
                        // SVG图片
                        .child(
                            img(page.path.clone())
                            // 设置宽度
                            .w(px(zoomed_width))
                            // 设置高度
                            .h(px(zoomed_height)),
                        ),
                    ),
                )
                // 转换为任意元素
                .into_any_element();
        }

        // 如果没有预览页面，显示提示信息
        let message = if self.render_phase == RenderPhase::Error {
            "No SVG preview is available because the latest render failed."
        } else {
            "Render the selected score to generate an SVG preview here."
        };

        // 创建空预览区域
        div()
            .flex_1()
            .min_h(px(0.0))
            .items_center()
            .justify_center()
            // 柔和文字颜色
            .text_color(cx.theme().muted_foreground)
            .text_size(px(14.0))
            .border_1()
            // 边框颜色
            .border_color(cx.theme().border)
            // 背景色
            .bg(cx.theme().background)
            // 提示信息
            .child(message)
            // 转换为任意元素
            .into_any_element()
    }

    // 创建渲染日志面板
    fn render_log_panel(&self, cx: &App) -> AnyElement {
        // 根据渲染状态设置标题
        let title = if self.render_phase == RenderPhase::Error {
            "LilyPond error output"
        } else {
            "LilyPond output"
        };

        // 创建日志面板
        div()
            // 使用弹性布局
            .flex()
            // 垂直方向排列
            .flex_col()
            // 子元素之间的间距为2单位
            .gap_2()
            // 固定高度为150像素
            .h(px(150.0))
            // 最小高度为150像素
            .min_h(px(150.0))
            // 内边距为3单位
            .p_3()
            // 设置背景色为主题中的柔和色调
            .bg(cx.theme().muted)
            // 边框宽度为1单位
            .border_1()
            // 边框颜色为主题中的边框颜色
            .border_color(cx.theme().border)
             // 标题容器
            .child(
                div()
                // 字体大小为12像素
                .text_size(px(12.0))
                // 字体粗细为半粗体
                .font_weight(FontWeight::SEMIBOLD)
                // 标题文本
                .child(title),
            )
            .child(
                // 日志内容区域
                div()
                // 弹性布局，占据剩余空间
                .flex_1()
                // 最小高度为0
                .min_h(px(0.0))
                // 垂直方向滚动条
                .overflow_y_scrollbar()
                // 字体族设置为等宽字体
                .font_family(cx.theme().mono_font_family.clone())
                // 字体大小设置为等宽字体大小
                .text_size(cx.theme().mono_font_size)
                //字体颜色设置为前景色
                .text_color(cx.theme().foreground)
                // 日志内容
                .child(self.render_log.clone()),
            )
            // 转换为任意元素
            .into_any_element()
    }
}

// 实现 GPUI 的 Render trait
impl Render for StudioApp {
    // 渲染方法，返回可转换为元素的类型
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 获取当前视图的实体（用于事件处理）
        let view = cx.entity();
        // 检查是否正在渲染
        let render_is_running = self.render_phase == RenderPhase::Rendering;
        // 检查是否有预览页面
        let has_preview = !self.preview_pages.is_empty();

        // 渲染按钮
        let render_button = Button::new("render-score")
            // 设置为主要按钮样式
            .primary()
            .label(if render_is_running {
                "Rendering..."
            } else {
                "Render SVG"
            })
        // 设置加载状态指示器
        .loading(render_is_running)
            // 渲染时禁用按钮
            .disabled(render_is_running)
            .on_click({
                // 克隆视图引用
                let view = view.clone();
                move |_, _, cx| {
                    // 开始渲染
                    let _ = view.update(cx, |studio, cx| studio.begin_render(cx));  
                }
            });

        // 上一页按钮
        let previous_page = Button::new("previous-preview-page")
            .label("Previous Page")
            // 在第一页或没有预览时禁用
            .disabled(self.current_preview_page == 0 || !has_preview)  
            .on_click({
                // 克隆视图引用
                let view = view.clone();
                move |_, _, cx| {
                    // 切换到上一页
                    let _ = view.update(cx, |studio, cx| studio.previous_preview_page(cx));
                }
            });

        // 下一页按钮
        let next_page = Button::new("next-preview-page")
            .label("Next Page")
            // 在最后一页或没有预览时禁用
            .disabled(!has_preview || self.current_preview_page + 1 >= self.preview_pages.len())
            .on_click({
                // 克隆视图引用
                let view = view.clone();
                move |_, _, cx| {
                    // 切换到下一页
                    let _ = view.update(cx, |studio, cx| studio.next_preview_page(cx));
                }
            });

        // 缩小按钮
        let zoom_out = Button::new("preview-zoom-out")
            .label("-")
            // 没有预览或达到最小缩放时禁用
            .disabled(!has_preview || self.preview_zoom <= Self::MIN_PREVIEW_ZOOM)
            .on_click({
                // 克隆视图引用
                let view = view.clone();
                move |_, _, cx| {
                    // 缩小预览
                    let _ = view.update(cx, |studio, cx| studio.zoom_out_preview(cx));
                }
            });

        // 重置缩放按钮
        let reset_zoom = Button::new("preview-zoom-reset")
            // 显示当前缩放比例
            .label(self.preview_zoom_label())
            // 没有预览时禁用
            .disabled(!has_preview)
            .on_click({
                // 克隆视图引用
                let view = view.clone();
                move |_, _, cx| {
                    // 重置缩放
                    let _ = view.update(cx, |studio, cx| studio.reset_preview_zoom(cx));
                }
            });

        // 放大按钮
        let zoom_in = Button::new("preview-zoom-in")
            .label("+")
            // 没有预览或达到最大缩放时禁用
            .disabled(!has_preview || self.preview_zoom >= Self::MAX_PREVIEW_ZOOM)
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    // 放大预览
                    let _ = view.update(cx, |studio, cx| studio.zoom_in_preview(cx));
                }
            });

        // 预览页面元信息
        let preview_meta = if self.preview_pages.is_empty() {
            "No pages yet".to_string()
        } else {
            format!(
                "Page {} of {}",
                self.current_preview_page + 1,
                self.preview_pages.len()
            )
        };

        // 根据当前屏幕选择要显示的内容
        let body = match self.current_screen {
            // 工作室屏幕
            AppScreen::Studio => self.studio_screen(
                cx,
                previous_page,
                next_page,
                zoom_out,
                reset_zoom,
                zoom_in,
                preview_meta,
            ),
            // 乐谱库屏幕
            AppScreen::Scores => div()
                // 弹性布局
                .flex()
                // 占据剩余空间
                .flex_1()
                // 最小高度
                .min_h(px(0.0))
                // 内边距
                .p_4()
                // 子元素为乐谱库屏幕
                .child(self.score_library_screen(cx))
                // 转换为任意元素
                .into_any_element(),
        };
        // 获取当前主题
        let theme = cx.theme();

        // 构建完整的应用界面
        div()
            // 弹性布局
            .flex()
            // 垂直方向排列
            .flex_col()
            // 占满父容器大小
            .size_full()
            // 设置背景色
            .bg(theme.background)
            // 设置文字颜色
            .text_color(theme.foreground)
            // 设置字体族
            .font_family(theme.font_family.clone())
            // 应用头部
            .child(self.app_header(view.clone(), render_button, cx))
            // 主体内容
            .child(body)
    }
}
