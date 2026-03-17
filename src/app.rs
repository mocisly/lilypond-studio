use crate::render::{RenderOutcome, RenderedPage, render_score};
use crate::tutorial::{LESSONS, TutorialLesson, default_source};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Application, Bounds, Context, Entity, FontWeight, KeyBinding, Menu,
    MenuItem, SharedString, Subscription, Window, WindowBounds, WindowOptions, actions, div,
    img, px, size,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::text::TextView;
use gpui_component::{ActiveTheme, Disableable, Icon, IconName, Root, Sizable, TitleBar};
use std::path::PathBuf;

actions!(studio, [Quit]);

pub fn run(render_root: PathBuf) {
    Application::new().run(move |cx: &mut App| {
        gpui_component::init(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("ctrl-q", Quit, None),
        ]);
        cx.set_menus(vec![Menu {
            name: "LilyPond Studio".into(),
            items: vec![MenuItem::action("Quit", Quit)],
        }]);

        let bounds = Bounds::centered(None, size(px(1440.0), px(940.0)), cx);
        #[cfg(target_os = "linux")]
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..Default::default()
        };
        #[cfg(not(target_os = "linux"))]
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            ..Default::default()
        };
        cx.open_window(
            window_options,
            {
                let render_root = render_root.clone();
                move |window, cx| {
                    let studio = cx.new(|cx| StudioApp::new(render_root.clone(), window, cx));
                    let _ = studio.update(cx, |studio, cx| studio.begin_render(cx));
                    cx.new(|cx| Root::new(studio, window, cx))
                }
            },
        )
        .expect("failed to open the LilyPond Studio window");

        cx.activate(true);
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderPhase {
    Idle,
    Rendering,
    Ready,
    Error,
}

struct StudioApp {
    editor: Entity<InputState>,
    tutorial_index: usize,
    render_phase: RenderPhase,
    render_root: PathBuf,
    next_render_job: u64,
    active_render_job: Option<u64>,
    preview_pages: Vec<RenderedPage>,
    current_preview_page: usize,
    preview_zoom: f32,
    render_log: SharedString,
    dirty: bool,
    _subscriptions: Vec<Subscription>,
}

impl StudioApp {
    const DEFAULT_PREVIEW_ZOOM: f32 = 1.0;
    const MIN_PREVIEW_ZOOM: f32 = 0.5;
    const MAX_PREVIEW_ZOOM: f32 = 3.0;
    const PREVIEW_ZOOM_STEP: f32 = 0.25;

    fn new(render_root: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("lilypond")
                .line_number(true)
                .rows(28)
                .soft_wrap(false)
                .placeholder("Write LilyPond notation here and render it to SVG.")
                .default_value(default_source())
        });

        let subscriptions =
            vec![
                cx.subscribe_in(&editor, window, |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.dirty = true;
                        cx.notify();
                    }
                }),
            ];

        Self {
            editor,
            tutorial_index: 0,
            render_phase: RenderPhase::Idle,
            render_root,
            next_render_job: 0,
            active_render_job: None,
            preview_pages: Vec::new(),
            current_preview_page: 0,
            preview_zoom: Self::DEFAULT_PREVIEW_ZOOM,
            render_log: SharedString::default(),
            dirty: true,
            _subscriptions: subscriptions,
        }
    }

    fn current_lesson(&self) -> &'static TutorialLesson {
        &LESSONS[self.tutorial_index]
    }

    fn begin_render(&mut self, cx: &mut Context<Self>) {
        let source = self.editor.read(cx).value().to_string();
        let job_id = self.next_render_job;
        let render_root = self.render_root.clone();

        self.next_render_job += 1;
        self.active_render_job = Some(job_id);
        self.render_phase = RenderPhase::Rendering;
        self.render_log = SharedString::default();
        self.dirty = false;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { render_score(&source, &render_root, job_id) })
                .await;

            let _ = entity.update(cx, |studio, cx| studio.finish_render(job_id, result, cx));
        })
        .detach();
    }

    fn finish_render(
        &mut self,
        job_id: u64,
        result: anyhow::Result<RenderOutcome>,
        cx: &mut Context<Self>,
    ) {
        if self.active_render_job != Some(job_id) {
            return;
        }

        self.active_render_job = None;

        match result {
            Ok(outcome) => {
                self.preview_pages = outcome.pages;
                self.current_preview_page = 0;
                self.render_phase = RenderPhase::Ready;
                self.render_log = outcome.log.into();
            }
            Err(err) => {
                self.render_phase = RenderPhase::Error;
                self.render_log = format!("{err:#}").into();
            }
        }

        cx.notify();
    }

    fn load_current_lesson_example(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lesson = self.current_lesson();
        let _ = self.editor.update(cx, |editor, cx| {
            editor.set_value(lesson.example, window, cx)
        });
        self.dirty = true;
        cx.notify();
    }

    fn previous_lesson(&mut self, cx: &mut Context<Self>) {
        if self.tutorial_index > 0 {
            self.tutorial_index -= 1;
            cx.notify();
        }
    }

    fn next_lesson(&mut self, cx: &mut Context<Self>) {
        if self.tutorial_index + 1 < LESSONS.len() {
            self.tutorial_index += 1;
            cx.notify();
        }
    }

    fn previous_preview_page(&mut self, cx: &mut Context<Self>) {
        if self.current_preview_page > 0 {
            self.current_preview_page -= 1;
            cx.notify();
        }
    }

    fn next_preview_page(&mut self, cx: &mut Context<Self>) {
        if self.current_preview_page + 1 < self.preview_pages.len() {
            self.current_preview_page += 1;
            cx.notify();
        }
    }

    fn zoom_out_preview(&mut self, cx: &mut Context<Self>) {
        self.preview_zoom =
            (self.preview_zoom - Self::PREVIEW_ZOOM_STEP).max(Self::MIN_PREVIEW_ZOOM);
        cx.notify();
    }

    fn zoom_in_preview(&mut self, cx: &mut Context<Self>) {
        self.preview_zoom =
            (self.preview_zoom + Self::PREVIEW_ZOOM_STEP).min(Self::MAX_PREVIEW_ZOOM);
        cx.notify();
    }

    fn reset_preview_zoom(&mut self, cx: &mut Context<Self>) {
        self.preview_zoom = Self::DEFAULT_PREVIEW_ZOOM;
        cx.notify();
    }

    fn preview_zoom_label(&self) -> String {
        format!("{:.0}%", self.preview_zoom * 100.0)
    }

    fn preview_page_summary(&self) -> String {
        match self.preview_pages.len() {
            0 => "No preview".to_string(),
            1 => "1 page".to_string(),
            count => format!("{count} pages"),
        }
    }

    fn header_chip(&self, icon: IconName, label: impl Into<SharedString>, cx: &App) -> AnyElement {
        let label: SharedString = label.into();

        div()
            .flex()
            .items_center()
            .gap_1()
            .h(px(22.0))
            .px_2()
            .bg(cx.theme().secondary)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(px(999.0))
            .child(
                Icon::new(icon)
                    .small()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .into_any_element()
    }

    fn app_header(&self, render_button: Button, cx: &App) -> AnyElement {
        TitleBar::new()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(22.0))
                            .h(px(22.0))
                            .rounded(px(7.0))
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .child("LP"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("LilyPond Studio"),
                            )
                            .child(
                                div()
                                    .h(px(20.0))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .rounded(px(999.0))
                                    .bg(cx.theme().secondary)
                                    .text_size(px(11.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Render session"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pr(px(12.0))
                    .child(self.header_chip(
                        IconName::BookOpen,
                        format!("Lesson {} of {}", self.tutorial_index + 1, LESSONS.len()),
                        cx,
                    ))
                    .child(self.header_chip(
                        IconName::File,
                        self.preview_page_summary(),
                        cx,
                    ))
                    .child(self.render_badge(cx))
                    .child(render_button.compact()),
            )
            .into_any_element()
    }

    fn render_badge(&self, cx: &App) -> AnyElement {
        let (label, bg, fg) = match self.render_phase {
            RenderPhase::Idle => (
                "Ready to render",
                cx.theme().muted,
                cx.theme().muted_foreground,
            ),
            RenderPhase::Rendering => ("Rendering", cx.theme().info, cx.theme().info_foreground),
            RenderPhase::Ready if self.dirty => (
                "Preview stale",
                cx.theme().warning,
                cx.theme().warning_foreground,
            ),
            RenderPhase::Ready => (
                "Preview current",
                cx.theme().success,
                cx.theme().success_foreground,
            ),
            RenderPhase::Error => (
                "Render failed",
                cx.theme().danger,
                cx.theme().danger_foreground,
            ),
        };

        div()
            .px_2()
            .py_1()
            .rounded(px(999.0))
            .bg(bg)
            .text_color(fg)
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .child(label)
            .into_any_element()
    }

    fn render_detail(&self) -> String {
        match self.render_phase {
            RenderPhase::Idle => {
                "Edit the score on the left, then compile it with the LilyPond CLI.".to_string()
            }
            RenderPhase::Rendering => {
                "Running LilyPond and collecting the generated SVG pages.".to_string()
            }
            RenderPhase::Ready if self.dirty => format!(
                "{} page(s) rendered. The editor changed since the last successful render.",
                self.preview_pages.len()
            ),
            RenderPhase::Ready => format!(
                "{} page(s) rendered from the current editor content.",
                self.preview_pages.len()
            ),
            RenderPhase::Error => {
                "LilyPond returned an error. Fix the notation and render again.".to_string()
            }
        }
    }

    fn preview_surface(&self, cx: &App) -> AnyElement {
        if let Some(page) = self.preview_pages.get(self.current_preview_page) {
            let zoomed_width = page.width_px * self.preview_zoom;
            let zoomed_height = page.height_px * self.preview_zoom;

            return div()
                .flex_1()
                .min_h(px(0.0))
                .bg(gpui::white())
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div().size_full().overflow_scrollbar().child(
                        div()
                            .p_3()
                            .w(px(zoomed_width + 24.0))
                            .h(px(zoomed_height + 24.0))
                            .child(
                                img(page.path.clone())
                                    .w(px(zoomed_width))
                                    .h(px(zoomed_height)),
                            ),
                    ),
                )
                .into_any_element();
        }

        let message = if self.render_phase == RenderPhase::Error {
            "No SVG preview is available because the latest render failed."
        } else {
            "Render the score to generate an SVG preview here."
        };

        div()
            .flex_1()
            .min_h(px(0.0))
            .items_center()
            .justify_center()
            .text_color(cx.theme().muted_foreground)
            .text_size(px(14.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(message)
            .into_any_element()
    }

    fn render_log_panel(&self, cx: &App) -> AnyElement {
        let title = if self.render_phase == RenderPhase::Error {
            "LilyPond error output"
        } else {
            "LilyPond output"
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .h(px(150.0))
            .min_h(px(150.0))
            .p_3()
            .bg(cx.theme().muted)
            .border_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(cx.theme().mono_font_size)
                    .text_color(cx.theme().foreground)
                    .child(self.render_log.clone()),
            )
            .into_any_element()
    }
}

impl Render for StudioApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let theme = cx.theme();
        let lesson = self.current_lesson();
        let render_is_running = self.render_phase == RenderPhase::Rendering;
        let has_preview = !self.preview_pages.is_empty();

        let render_button = Button::new("render-score")
            .primary()
            .label(if render_is_running {
                "Rendering..."
            } else {
                "Render SVG"
            })
            .loading(render_is_running)
            .disabled(render_is_running)
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.begin_render(cx));
                }
            });

        let previous_page = Button::new("previous-preview-page")
            .label("Previous Page")
            .disabled(self.current_preview_page == 0 || !has_preview)
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.previous_preview_page(cx));
                }
            });

        let next_page = Button::new("next-preview-page")
            .label("Next Page")
            .disabled(!has_preview || self.current_preview_page + 1 >= self.preview_pages.len())
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.next_preview_page(cx));
                }
            });

        let zoom_out = Button::new("preview-zoom-out")
            .label("-")
            .disabled(!has_preview || self.preview_zoom <= Self::MIN_PREVIEW_ZOOM)
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.zoom_out_preview(cx));
                }
            });

        let reset_zoom = Button::new("preview-zoom-reset")
            .label(self.preview_zoom_label())
            .disabled(!has_preview)
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.reset_preview_zoom(cx));
                }
            });

        let zoom_in = Button::new("preview-zoom-in")
            .label("+")
            .disabled(!has_preview || self.preview_zoom >= Self::MAX_PREVIEW_ZOOM)
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.zoom_in_preview(cx));
                }
            });

        let previous_lesson = Button::new("previous-lesson")
            .label("Previous")
            .disabled(self.tutorial_index == 0)
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.previous_lesson(cx));
                }
            });

        let next_lesson = Button::new("next-lesson")
            .label("Next")
            .disabled(self.tutorial_index + 1 >= LESSONS.len())
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |studio, cx| studio.next_lesson(cx));
                }
            });

        let load_example = Button::new("load-lesson-example")
            .label("Load Example")
            .on_click({
                let view = view.clone();
                move |_, window, cx| {
                    let _ = view.update(cx, |studio, cx| {
                        studio.load_current_lesson_example(window, cx)
                    });
                }
            });

        let preview_meta = if self.preview_pages.is_empty() {
            "No pages yet".to_string()
        } else {
            format!(
                "Page {} of {}",
                self.current_preview_page + 1,
                self.preview_pages.len()
            )
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .child(self.app_header(render_button, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .gap_4()
                    .p_4()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_4()
                            .bg(theme.secondary)
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Score Editor"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(theme.muted_foreground)
                                            .child(
                                                "The editor is a GPUI-native multiline input. `Cmd/Ctrl+F` opens in-editor search, and each render writes a fresh LilyPond job workspace.",
                                            ),
                                    ),
                            )
                            .child(div().flex_1().min_h(px(0.0)).child(Input::new(&self.editor).h_full())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .gap_3()
                                    .p_4()
                                    .bg(theme.secondary)
                                    .border_1()
                                    .border_color(theme.border)
                                    .child(
                                        div()
                                            .flex()
                                            .items_start()
                                            .justify_between()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_size(px(16.0))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child("SVG Preview"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .text_color(theme.muted_foreground)
                                                            .child(self.render_detail()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(zoom_out)
                                                    .child(reset_zoom)
                                                    .child(zoom_in)
                                                    .child(previous_page)
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .text_color(theme.muted_foreground)
                                                            .child(preview_meta),
                                                    )
                                                    .child(next_page),
                                            ),
                                    )
                                    .child(self.preview_surface(cx))
                                    .when(!self.render_log.is_empty(), |this| {
                                        this.child(self.render_log_panel(cx))
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .h(px(360.0))
                                    .min_h(px(300.0))
                                    .gap_3()
                                    .p_4()
                                    .bg(theme.secondary)
                                    .border_1()
                                    .border_color(theme.border)
                                    .child(
                                        div()
                                            .flex()
                                            .items_start()
                                            .justify_between()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_size(px(16.0))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child("Tutorial"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .text_color(theme.muted_foreground)
                                                            .child(format!(
                                                                "Lesson {} of {}: {}",
                                                                self.tutorial_index + 1,
                                                                LESSONS.len(),
                                                                lesson.title
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .text_color(theme.muted_foreground)
                                                            .child(lesson.summary),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(previous_lesson)
                                                    .child(next_lesson)
                                                    .child(load_example),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_h(px(0.0))
                                            .overflow_hidden()
                                            .child(
                                                TextView::markdown(
                                                    "tutorial-markdown",
                                                    lesson.markdown,
                                                    window,
                                                    cx,
                                                )
                                                .scrollable(true)
                                                .selectable(true)
                                                .size_full(),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
