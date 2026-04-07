// 导入依赖
use anyhow::{Context, Result, bail};                            // 错误处理库
use async_std::task;                                            // 异步运行时
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};    // SQLite 连接选项和连接池
use sqlx::{Row, SqlitePool};                                    // SQLite 连接池和行访问
use std::future::Future;                                        // Future trait
use std::path::{Path, PathBuf};                                 // 路径处理
use std::str::FromStr;                                          // 字符串解析
use std::sync::Arc;                                             // 原子引用计数

use crate::tutorial::default_source;                            // 导入教程模块的默认源码

// 类型别名
pub type ScoreId = i64;  // 乐谱 ID 类型

// 乐谱记录结构体
#[derive(Clone, Debug, PartialEq, Eq)]  // 实现克隆、调试、相等性比较
pub struct ScoreRecord {
    pub id: ScoreId,                    // 乐谱唯一标识符
    pub title: String,                  // 乐谱标题
    pub source: String,                 // LilyPond 源代码
}

// 乐谱存储 trait，定义数据存储接口
pub trait ScoreStore: Send + Sync {                                             // Send + Sync 允许跨线程使用
    fn list_scores(&self) -> Result<Vec<ScoreRecord>>;                          // 列出所有乐谱
    fn create_score(&self, title: &str, source: &str) -> Result<ScoreRecord>;   // 创建新乐谱
    fn update_score_title(&self, id: ScoreId, title: &str) -> Result<()>;       // 更新乐谱标题
    fn update_score_source(&self, id: ScoreId, source: &str) -> Result<()>;     // 更新乐谱源码
    fn delete_score(&self, id: ScoreId) -> Result<()>;                          // 删除乐谱
}

// SQLite 实现
pub struct SqliteScoreStore {
    pool: SqlitePool,           // SQLite 连接池
}

impl SqliteScoreStore {
    // 打开或创建数据库
    pub fn open(path: &Path) -> Result<Self> {
        // 确保数据库目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create the score library directory at {}",
                    parent.display()
                )
            })?;
        }

        // 构建 SQLite 连接选项
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            // 解析连接字符串
            .context("failed to build the SQLite connection options")?
            // 数据库不存在时自动创建
            .create_if_missing(true);


        // 创建连接池
        let pool = block_on(
            SqlitePoolOptions::new()
                // 限制最大连接数为1
                .max_connections(1)
                // 使用选项连接
                .connect_with(options),
        )
        .context("failed to connect to the score library database")?;

        // 初始化数据库表结构
        block_on(
            sqlx::query(
                r#"
            CREATE TABLE IF NOT EXISTS scores (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                source TEXT NOT NULL
            )
            "#,
            )
            // 执行创建表语句
            .execute(&pool),
        )
        .context("failed to initialize the score library schema")?;

        // 返回存储实例
        Ok(Self { pool })
    }
}

impl ScoreStore for SqliteScoreStore {
    fn list_scores(&self) -> Result<Vec<ScoreRecord>> {
        // 执行查询，获取所有乐谱记录
        let rows = block_on(
            // 获取所有结果
            sqlx::query("SELECT id, title, source FROM scores ORDER BY id ASC")
                .fetch_all(&self.pool),
        )
        .context("failed to load scores from SQLite")?;

        // 将查询结果转换为 ScoreRecord
        rows.into_iter()
            .map(|row| {
                Ok(ScoreRecord {
                    // 获取 id 字段
                    id: row.try_get("id")?,
                    // 获取 title 字段
                    title: row.try_get("title")?,
                    // 获取 source 字段
                    source: row.try_get("source")?,
                })
            })
            // 收集为 Vec<ScoreRecord>
            .collect()
    }

    fn create_score(&self, title: &str, source: &str) -> Result<ScoreRecord> {
        // 插入新乐谱记录
        let id = block_on(
            sqlx::query("INSERT INTO scores (title, source) VALUES (?, ?)")
                // 绑定标题参数
                .bind(title)
                // 绑定源码参数
                .bind(source)
                // 执行插入
                .execute(&self.pool),
        )
        .context("failed to insert a score into SQLite")?
        // 获取最后插入的 ID
        .last_insert_rowid();

        // 返回创建的乐谱记录
        Ok(ScoreRecord {
            id,
            title: title.to_string(),
            source: source.to_string(),
        })
    }

    fn update_score_title(&self, id: ScoreId, title: &str) -> Result<()> {
        // 更新乐谱标题
        let rows_affected = block_on(
            sqlx::query("UPDATE scores SET title = ? WHERE id = ?")
                // 绑定新标题
                .bind(title)
                // 绑定乐谱 ID
                .bind(id)
                // 执行更新
                .execute(&self.pool),
        )
        .context("failed to rename a score in SQLite")?
        // 获取受影响的行数
        .rows_affected();

        // 检查是否成功更新
        if rows_affected == 0 {
            bail!("score {id} was not found while renaming");
        }

        Ok(())
    }

    fn update_score_source(&self, id: ScoreId, source: &str) -> Result<()> {
        // 更新乐谱源码
        let rows_affected = block_on(
            sqlx::query("UPDATE scores SET source = ? WHERE id = ?")
                .bind(source)  // 绑定新源码
                .bind(id)  // 绑定乐谱 ID
                .execute(&self.pool),  // 执行更新
        )
        .context("failed to update a score source in SQLite")?
        .rows_affected();  // 获取受影响的行数

        // 检查是否成功更新
        if rows_affected == 0 {
            bail!("score {id} was not found while saving");
        }

        Ok(())
    }

    fn delete_score(&self, id: ScoreId) -> Result<()> {
        // 删除乐谱记录
        let rows_affected = block_on(
            sqlx::query("DELETE FROM scores WHERE id = ?")
                .bind(id)  // 绑定乐谱 ID
                .execute(&self.pool),  // 执行删除
        )
        .context("failed to delete a score from SQLite")?
        .rows_affected();  // 获取受影响的行数

        // 检查是否成功删除
        if rows_affected == 0 {
            bail!("score {id} was not found while deleting");
        }

        Ok(())
    }
}

// 乐谱管理器
pub struct ScoreManager {
    store: Arc<dyn ScoreStore>,         // 存储后端
    scores: Vec<ScoreRecord>,           // 内存中的乐谱列表
    selected_score_id: ScoreId,         // 当前选中的乐谱 ID
    next_new_score_index: u32,          // 下一个新乐谱的索引
}

impl ScoreManager {
    // 加载乐谱管理器
    pub fn load(store: Arc<dyn ScoreStore>) -> Result<Self> {
        // 从存储中获取所有乐谱
        let mut scores = store.list_scores()?;
        
        // 如果没有乐谱，创建默认乐谱
        if scores.is_empty() {
            store.create_score(DEFAULT_SCORE_TITLE, default_source())?;
            scores = store.list_scores()?;
        }

        // 设置默认选中的乐谱
        let selected_score_id = scores
            .first()
            .map(|score| score.id)
            .context("score library failed to produce an initial score")?;

        // 计算下一个新乐谱的索引
        let next_new_score_index = scores
            .iter()
            // 解析标题中的索引
            .filter_map(|score| parse_untitled_index(&score.title))
            .max()
            // 如果没有找到，从0开始
            .unwrap_or(0)
            // 下一个索引
            + 1;

        // 返回乐谱管理器实例
        Ok(Self {
            store,
            scores,
            selected_score_id,
            next_new_score_index,
        })
    }

    // 获取所有乐谱
    pub fn scores(&self) -> &[ScoreRecord] {
        &self.scores
    }

    // 获取当前选中的乐谱
    pub fn selected_score(&self) -> &ScoreRecord {
        self.scores
            .iter()
            // 查找选中的乐谱
            .find(|score| score.id == self.selected_score_id)
            // 应该始终存在
            .expect("selected score should always exist")
    }

    // 获取当前选中的乐谱 ID
    pub fn selected_score_id(&self) -> ScoreId {
        self.selected_score_id
    }

    // 选择乐谱
    pub fn select_score(&mut self, id: ScoreId) -> bool {
        // 如果已经是选中的乐谱，或者乐谱不存在，返回 false
        if self.selected_score_id == id || !self.scores.iter().any(|score| score.id == id) {
            return false;
        }

        // 更新选中的乐谱 ID
        self.selected_score_id = id;
        true
    }

    // 创建新乐谱
    pub fn create_score(&mut self) -> Result<ScoreRecord> {
        // 生成新乐谱标题
        let title = format!("Untitled Score {}", self.next_new_score_index);
        // 递增索引
        self.next_new_score_index += 1;
        
        // 在存储中创建乐谱
        let record = self.store.create_score(&title, NEW_SCORE_SOURCE)?;
        
        // 更新当前选中的乐谱
        self.selected_score_id = record.id;
        // 添加到内存列表
        self.scores.push(record.clone());
        
        Ok(record)
    }

    // 重命名选中的乐谱
    pub fn rename_selected_score(&mut self, title: impl Into<String>) -> Result<()> {
        let title = title.into();
        let trimmed = title.trim();
        
        // 如果标题为空，不做任何操作
        if trimmed.is_empty() {
            return Ok(());
        }

        // 获取当前选中的乐谱 ID
        let selected_id = self.selected_score_id;
        
        // 更新存储中的标题
        self.store.update_score_title(selected_id, trimmed)?;
        
        // 更新内存中的标题
        if let Some(score) = self.scores.iter_mut().find(|score| score.id == selected_id) {
            score.title = trimmed.to_string();
        }
        
        Ok(())
    }

    // 更新选中乐谱的源码
    pub fn update_selected_source(&mut self, source: impl Into<String>) -> Result<()> {
        let source = source.into();
        
        // 获取当前选中的乐谱 ID
        let selected_id = self.selected_score_id;
        
        // 更新存储中的源码
        self.store.update_score_source(selected_id, &source)?;
        
        // 更新内存中的源码
        if let Some(score) = self.scores.iter_mut().find(|score| score.id == selected_id) {
            score.source = source;
        }
        
        Ok(())
    }

    // 删除选中的乐谱
    pub fn delete_selected_score(&mut self) -> Result<()> {
        // 获取当前选中的乐谱 ID
        let selected_id = self.selected_score_id;
        
        // 查找在内存中的索引位置
        let selected_index = self
            .scores
            .iter()
            .position(|score| score.id == selected_id)
            .context("selected score should exist before deletion")?;
        
        // 从存储中删除
        self.store.delete_score(selected_id)?;
        // 从内存中移除
        self.scores.remove(selected_index);

        // 如果没有乐谱了，创建默认乐谱
        if self.scores.is_empty() {
            let record = self
                .store
                .create_score(DEFAULT_SCORE_TITLE, default_source())?;
            self.selected_score_id = record.id;
            self.scores.push(record);
            return Ok(());
        }

        // 选择下一个乐谱
        let next_index = selected_index.min(self.scores.len() - 1);
        self.selected_score_id = self.scores[next_index].id;
        
        Ok(())
    }
}

// 默认乐谱标题
const DEFAULT_SCORE_TITLE: &str = "Starter Score";

// 新乐谱的默认源码
const NEW_SCORE_SOURCE: &str = r#"\version "2.24.0"

\header {
  title = "Untitled Score"
}

\score {
  \new Staff \relative c' {
    \key c \major
    \time 4/4

    c4 d e f
    g1
  }

  \layout { }
}"#;

// 从标题中解析无标题乐谱的索引
fn parse_untitled_index(title: &str) -> Option<u32> {
    title
        // 检查是否以 "Untitled Score " 开头
        .strip_prefix("Untitled Score ")
        // 解析为数字
        .and_then(|value| value.parse::<u32>().ok())
}

// 异步阻塞执行
fn block_on<F>(future: F) -> F::Output
where
    // 接收任何 Future
    F: Future,
{
    // 阻塞执行异步任务
    task::block_on(future)
}

// 获取默认数据库路径
pub fn default_database_path() -> PathBuf {
    // 在应用数据目录下创建 scores.db
    app_data_root().join("scores.db")
}

// 获取应用数据根目录
fn app_data_root() -> PathBuf {
    // Windows 系统
    if cfg!(target_os = "windows") {
        if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(path).join("lilypond-studio");
        }
    }

    // macOS 系统
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("lilypond-studio");
        }
    }

    // Linux/Unix 系统，先尝试 XDG_DATA_HOME
    if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(path).join("lilypond-studio");
    }

    // 回退到用户主目录
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("lilypond-studio");
    }

    // 如果都失败了，使用临时目录
    std::env::temp_dir().join("lilypond-studio")
}

// 测试模块
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;                           // 互斥锁
    use std::time::{SystemTime, UNIX_EPOCH};        // 时间处理

    // 内存存储实现，用于测试
    #[derive(Default)]
    struct InMemoryScoreStore {
        inner: Mutex<InMemoryState>,                // 内部状态，使用互斥锁保护
    }

    #[derive(Default)]
    struct InMemoryState {
        next_id: ScoreId,               // 下一个可用的 ID
        scores: Vec<ScoreRecord>,       // 存储的乐谱
    }

    impl InMemoryScoreStore {
        // 用初始乐谱列表创建内存存储
        fn with_scores(scores: Vec<ScoreRecord>) -> Self {
            let next_id = scores.iter().map(|score| score.id).max().unwrap_or(0);
            Self {
                inner: Mutex::new(InMemoryState { next_id, scores }),
            }
        }
    }

    impl ScoreStore for InMemoryScoreStore {
        fn list_scores(&self) -> Result<Vec<ScoreRecord>> {
            // 克隆列表返回
            Ok(self.inner.lock().unwrap().scores.clone())
        }

        fn create_score(&self, title: &str, source: &str) -> Result<ScoreRecord> {
            // 获取锁
            let mut inner = self.inner.lock().unwrap();
            // 递增 ID
            inner.next_id += 1;
            let score = ScoreRecord {
                id: inner.next_id,
                title: title.to_string(),
                source: source.to_string(),
            };
            // 添加到列表
            inner.scores.push(score.clone());
            Ok(score)
        }

        fn update_score_title(&self, id: ScoreId, title: &str) -> Result<()> {
            let mut inner = self.inner.lock().unwrap();
            // 查找并更新乐谱
            let score = inner
                .scores
                .iter_mut()
                .find(|score| score.id == id)
                .context("score should exist while renaming")?;
            score.title = title.to_string();
            Ok(())
        }

        fn update_score_source(&self, id: ScoreId, source: &str) -> Result<()> {
            let mut inner = self.inner.lock().unwrap();
            // 查找并更新乐谱
            let score = inner
                .scores
                .iter_mut()
                .find(|score| score.id == id)
                .context("score should exist while saving")?;
            score.source = source.to_string();
            Ok(())
        }

        fn delete_score(&self, id: ScoreId) -> Result<()> {
            let mut inner = self.inner.lock().unwrap();
            // 查找并删除乐谱
            let index = inner
                .scores
                .iter()
                .position(|score| score.id == id)
                .context("score should exist while deleting")?;
            inner.scores.remove(index);
            Ok(())
        }
    }

    // 辅助函数：创建乐谱记录
    fn record(id: ScoreId, title: &str, source: &str) -> ScoreRecord {
        ScoreRecord {
            id,
            title: title.to_string(),
            source: source.to_string(),
        }
    }

    // 测试：当库为空时创建默认乐谱
    #[test]
    fn seeds_default_score_when_library_is_empty() {
        // 创建空存储
        let store = Arc::new(InMemoryScoreStore::default());
        // 加载管理器
        let manager = ScoreManager::load(store).unwrap();

        // 应该有1个乐谱
        assert_eq!(manager.scores().len(), 1);
        // 标题应为默认标题
        assert_eq!(manager.selected_score().title, DEFAULT_SCORE_TITLE);
        // 源码应为默认源码
        assert_eq!(manager.selected_score().source, default_source());
    }

    // 测试：加载已存在的乐谱时不重新创建默认乐谱
    #[test]
    fn loads_existing_scores_without_reseeding() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(4, "Existing One", "a"),
            record(9, "Existing Two", "b"),
        ]));
        let manager = ScoreManager::load(store).unwrap();

        // 应该有两个乐谱
        assert_eq!(manager.scores().len(), 2);
        // 应该选择第一个乐谱
        assert_eq!(manager.selected_score_id(), 4);
    }

    // 测试：选择乐谱功能
    #[test]
    fn selecting_a_score_changes_the_current_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(1, "One", "a"),
            record(2, "Two", "b"),
        ]));
        let mut manager = ScoreManager::load(store).unwrap();

        // 选择第二个乐谱
        assert!(manager.select_score(2));
        // 验证标题
        assert_eq!(manager.selected_score().title, "Two");
    }

    // 测试：创建新乐谱
    #[test]
    fn creating_a_score_selects_and_appends_it() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![record(1, "One", "a")]));
        let mut manager = ScoreManager::load(store).unwrap();

        // 创建新乐谱
        let created = manager.create_score().unwrap();

        // 验证标题格式
        assert_eq!(created.title, "Untitled Score 1");
        // 应该选中新创建的乐谱
        assert_eq!(manager.selected_score_id(), created.id);
        // 新乐谱应该在列表末尾
        assert_eq!(manager.scores().last().unwrap().id, created.id);
    }

    // 测试：重命名选中的乐谱
    #[test]
    fn renaming_selected_score_only_updates_that_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(1, "One", "a"),
            record(2, "Two", "b"),
        ]));
        let mut manager = ScoreManager::load(store).unwrap();
        // 选择第二个乐谱
        manager.select_score(2);

        // 重命名
        manager.rename_selected_score("Renamed").unwrap();

        // 第一个乐谱不变
        assert_eq!(manager.scores()[0].title, "One");
        // 第二个乐谱被重命名
        assert_eq!(manager.scores()[1].title, "Renamed");
    }

    // 测试：删除选中的乐谱
    #[test]
    fn deleting_selected_score_moves_selection_to_the_next_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(1, "One", "a"),
            record(2, "Two", "b"),
            record(3, "Three", "c"),
        ]));
        let mut manager = ScoreManager::load(store).unwrap();
        // 选择第二个乐谱
        manager.select_score(2);

        // 删除选中的乐谱
        manager.delete_selected_score().unwrap();

        // 应该选中第三个乐谱
        assert_eq!(manager.selected_score_id(), 3);
        // 应该剩下2个乐谱
        assert_eq!(manager.scores().len(), 2);
    }

    // 测试：删除最后一个乐谱
    #[test]
    fn deleting_the_last_score_recreates_a_default_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![record(
            1, "Only", "a",
        )]));
        let mut manager = ScoreManager::load(store).unwrap();

        // 删除唯一的乐谱
        manager.delete_selected_score().unwrap();

        // 应该有一个乐谱
        assert_eq!(manager.scores().len(), 1);
        // 应该是默认乐谱
        assert_eq!(manager.selected_score().title, DEFAULT_SCORE_TITLE);
    }

    // 测试：更新选中的源码
    #[test]
    fn updating_selected_source_keeps_the_latest_editor_content() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![record(
            1, "One", "old",
        )]));
        let mut manager = ScoreManager::load(store).unwrap();

        // 更新源码
        manager.update_selected_source("new source").unwrap();

        // 源码应该被更新
        assert_eq!(manager.selected_score().source, "new source");
    }

    // 测试：SQLite 存储的完整流程
    #[test]
    fn sqlite_store_round_trips_scores() {
        // 创建唯一的测试数据库路径
        let path = unique_test_db_path("round-trip");
        let store = SqliteScoreStore::open(&path).unwrap();

        // 创建乐谱
        let created = store.create_score("Round Trip", "source").unwrap();
        let listed = store.list_scores().unwrap();

        // 应该有一个乐谱
        assert_eq!(listed.len(), 1);
        // ID 应该匹配
        assert_eq!(listed[0].id, created.id);

        // 更新标题和源码
        store.update_score_title(created.id, "Renamed").unwrap();
        store.update_score_source(created.id, "updated").unwrap();

        let updated = store.list_scores().unwrap();
        // 标题应该更新
        assert_eq!(updated[0].title, "Renamed");
        // 源码应该更新
        assert_eq!(updated[0].source, "updated");

        // 删除乐谱
        store.delete_score(created.id).unwrap();
        // 应该没有乐谱了
        assert!(store.list_scores().unwrap().is_empty());

        // 清理：删除测试数据库文件
        let _ = std::fs::remove_file(path);
    }

    // 测试：SQLite 架构初始化
    #[test]
    fn sqlite_store_initializes_schema_for_an_empty_database() {
        // 创建唯一的测试数据库路径
        let path = unique_test_db_path("schema");
        let store = SqliteScoreStore::open(&path).unwrap();

        // 数据库应该为空
        assert!(store.list_scores().unwrap().is_empty());

        // 清理：删除测试数据库文件
        let _ = std::fs::remove_file(path);
    }

    // 生成唯一的测试数据库路径
    fn unique_test_db_path(label: &str) -> PathBuf {
        // 获取纳秒级时间戳
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lilypond-studio-{label}-{nanos}.db"))
    }
}
