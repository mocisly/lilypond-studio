use anyhow::{Context, Result, bail};
use async_std::task;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tutorial::default_source;

pub type ScoreId = i64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreRecord {
    pub id: ScoreId,
    pub title: String,
    pub source: String,
}

pub trait ScoreStore: Send + Sync {
    fn list_scores(&self) -> Result<Vec<ScoreRecord>>;
    fn create_score(&self, title: &str, source: &str) -> Result<ScoreRecord>;
    fn update_score_title(&self, id: ScoreId, title: &str) -> Result<()>;
    fn update_score_source(&self, id: ScoreId, source: &str) -> Result<()>;
    fn delete_score(&self, id: ScoreId) -> Result<()>;
}

pub struct SqliteScoreStore {
    pool: SqlitePool,
}

impl SqliteScoreStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create the score library directory at {}",
                    parent.display()
                )
            })?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .context("failed to build the SQLite connection options")?
            .create_if_missing(true);

        let pool = block_on(
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options),
        )
        .context("failed to connect to the score library database")?;

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
            .execute(&pool),
        )
        .context("failed to initialize the score library schema")?;

        Ok(Self { pool })
    }
}

impl ScoreStore for SqliteScoreStore {
    fn list_scores(&self) -> Result<Vec<ScoreRecord>> {
        let rows = block_on(
            sqlx::query("SELECT id, title, source FROM scores ORDER BY id ASC")
                .fetch_all(&self.pool),
        )
        .context("failed to load scores from SQLite")?;

        rows.into_iter()
            .map(|row| {
                Ok(ScoreRecord {
                    id: row.try_get("id")?,
                    title: row.try_get("title")?,
                    source: row.try_get("source")?,
                })
            })
            .collect()
    }

    fn create_score(&self, title: &str, source: &str) -> Result<ScoreRecord> {
        let id = block_on(
            sqlx::query("INSERT INTO scores (title, source) VALUES (?, ?)")
                .bind(title)
                .bind(source)
                .execute(&self.pool),
        )
        .context("failed to insert a score into SQLite")?
        .last_insert_rowid();

        Ok(ScoreRecord {
            id,
            title: title.to_string(),
            source: source.to_string(),
        })
    }

    fn update_score_title(&self, id: ScoreId, title: &str) -> Result<()> {
        let rows_affected = block_on(
            sqlx::query("UPDATE scores SET title = ? WHERE id = ?")
                .bind(title)
                .bind(id)
                .execute(&self.pool),
        )
        .context("failed to rename a score in SQLite")?
        .rows_affected();

        if rows_affected == 0 {
            bail!("score {id} was not found while renaming");
        }

        Ok(())
    }

    fn update_score_source(&self, id: ScoreId, source: &str) -> Result<()> {
        let rows_affected = block_on(
            sqlx::query("UPDATE scores SET source = ? WHERE id = ?")
                .bind(source)
                .bind(id)
                .execute(&self.pool),
        )
        .context("failed to update a score source in SQLite")?
        .rows_affected();

        if rows_affected == 0 {
            bail!("score {id} was not found while saving");
        }

        Ok(())
    }

    fn delete_score(&self, id: ScoreId) -> Result<()> {
        let rows_affected = block_on(
            sqlx::query("DELETE FROM scores WHERE id = ?")
                .bind(id)
                .execute(&self.pool),
        )
        .context("failed to delete a score from SQLite")?
        .rows_affected();

        if rows_affected == 0 {
            bail!("score {id} was not found while deleting");
        }

        Ok(())
    }
}

pub struct ScoreManager {
    store: Arc<dyn ScoreStore>,
    scores: Vec<ScoreRecord>,
    selected_score_id: ScoreId,
    next_new_score_index: u32,
}

impl ScoreManager {
    pub fn load(store: Arc<dyn ScoreStore>) -> Result<Self> {
        let mut scores = store.list_scores()?;
        if scores.is_empty() {
            store.create_score(DEFAULT_SCORE_TITLE, default_source())?;
            scores = store.list_scores()?;
        }

        let selected_score_id = scores
            .first()
            .map(|score| score.id)
            .context("score library failed to produce an initial score")?;

        let next_new_score_index = scores
            .iter()
            .filter_map(|score| parse_untitled_index(&score.title))
            .max()
            .unwrap_or(0)
            + 1;

        Ok(Self {
            store,
            scores,
            selected_score_id,
            next_new_score_index,
        })
    }

    pub fn scores(&self) -> &[ScoreRecord] {
        &self.scores
    }

    pub fn selected_score(&self) -> &ScoreRecord {
        self.scores
            .iter()
            .find(|score| score.id == self.selected_score_id)
            .expect("selected score should always exist")
    }

    pub fn selected_score_id(&self) -> ScoreId {
        self.selected_score_id
    }

    pub fn select_score(&mut self, id: ScoreId) -> bool {
        if self.selected_score_id == id || !self.scores.iter().any(|score| score.id == id) {
            return false;
        }

        self.selected_score_id = id;
        true
    }

    pub fn create_score(&mut self) -> Result<ScoreRecord> {
        let title = format!("Untitled Score {}", self.next_new_score_index);
        self.next_new_score_index += 1;
        let record = self.store.create_score(&title, NEW_SCORE_SOURCE)?;
        self.selected_score_id = record.id;
        self.scores.push(record.clone());
        Ok(record)
    }

    pub fn rename_selected_score(&mut self, title: impl Into<String>) -> Result<()> {
        let title = title.into();
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let selected_id = self.selected_score_id;
        self.store.update_score_title(selected_id, trimmed)?;
        if let Some(score) = self.scores.iter_mut().find(|score| score.id == selected_id) {
            score.title = trimmed.to_string();
        }
        Ok(())
    }

    pub fn update_selected_source(&mut self, source: impl Into<String>) -> Result<()> {
        let source = source.into();
        let selected_id = self.selected_score_id;
        self.store.update_score_source(selected_id, &source)?;
        if let Some(score) = self.scores.iter_mut().find(|score| score.id == selected_id) {
            score.source = source;
        }
        Ok(())
    }

    pub fn delete_selected_score(&mut self) -> Result<()> {
        let selected_id = self.selected_score_id;
        let selected_index = self
            .scores
            .iter()
            .position(|score| score.id == selected_id)
            .context("selected score should exist before deletion")?;

        self.store.delete_score(selected_id)?;
        self.scores.remove(selected_index);

        if self.scores.is_empty() {
            let record = self
                .store
                .create_score(DEFAULT_SCORE_TITLE, default_source())?;
            self.selected_score_id = record.id;
            self.scores.push(record);
            return Ok(());
        }

        let next_index = selected_index.min(self.scores.len() - 1);
        self.selected_score_id = self.scores[next_index].id;
        Ok(())
    }
}

const DEFAULT_SCORE_TITLE: &str = "Starter Score";
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

fn parse_untitled_index(title: &str) -> Option<u32> {
    title
        .strip_prefix("Untitled Score ")
        .and_then(|value| value.parse::<u32>().ok())
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    task::block_on(future)
}

pub fn default_database_path() -> PathBuf {
    app_data_root().join("scores.db")
}

fn app_data_root() -> PathBuf {
    if cfg!(target_os = "windows") {
        if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(path).join("lilypond-studio");
        }
    }

    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("lilypond-studio");
        }
    }

    if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(path).join("lilypond-studio");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("lilypond-studio");
    }

    std::env::temp_dir().join("lilypond-studio")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct InMemoryScoreStore {
        inner: Mutex<InMemoryState>,
    }

    #[derive(Default)]
    struct InMemoryState {
        next_id: ScoreId,
        scores: Vec<ScoreRecord>,
    }

    impl InMemoryScoreStore {
        fn with_scores(scores: Vec<ScoreRecord>) -> Self {
            let next_id = scores.iter().map(|score| score.id).max().unwrap_or(0);
            Self {
                inner: Mutex::new(InMemoryState { next_id, scores }),
            }
        }
    }

    impl ScoreStore for InMemoryScoreStore {
        fn list_scores(&self) -> Result<Vec<ScoreRecord>> {
            Ok(self.inner.lock().unwrap().scores.clone())
        }

        fn create_score(&self, title: &str, source: &str) -> Result<ScoreRecord> {
            let mut inner = self.inner.lock().unwrap();
            inner.next_id += 1;
            let score = ScoreRecord {
                id: inner.next_id,
                title: title.to_string(),
                source: source.to_string(),
            };
            inner.scores.push(score.clone());
            Ok(score)
        }

        fn update_score_title(&self, id: ScoreId, title: &str) -> Result<()> {
            let mut inner = self.inner.lock().unwrap();
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
            let index = inner
                .scores
                .iter()
                .position(|score| score.id == id)
                .context("score should exist while deleting")?;
            inner.scores.remove(index);
            Ok(())
        }
    }

    fn record(id: ScoreId, title: &str, source: &str) -> ScoreRecord {
        ScoreRecord {
            id,
            title: title.to_string(),
            source: source.to_string(),
        }
    }

    #[test]
    fn seeds_default_score_when_library_is_empty() {
        let store = Arc::new(InMemoryScoreStore::default());
        let manager = ScoreManager::load(store).unwrap();

        assert_eq!(manager.scores().len(), 1);
        assert_eq!(manager.selected_score().title, DEFAULT_SCORE_TITLE);
        assert_eq!(manager.selected_score().source, default_source());
    }

    #[test]
    fn loads_existing_scores_without_reseeding() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(4, "Existing One", "a"),
            record(9, "Existing Two", "b"),
        ]));
        let manager = ScoreManager::load(store).unwrap();

        assert_eq!(manager.scores().len(), 2);
        assert_eq!(manager.selected_score_id(), 4);
    }

    #[test]
    fn selecting_a_score_changes_the_current_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(1, "One", "a"),
            record(2, "Two", "b"),
        ]));
        let mut manager = ScoreManager::load(store).unwrap();

        assert!(manager.select_score(2));
        assert_eq!(manager.selected_score().title, "Two");
    }

    #[test]
    fn creating_a_score_selects_and_appends_it() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![record(1, "One", "a")]));
        let mut manager = ScoreManager::load(store).unwrap();

        let created = manager.create_score().unwrap();

        assert_eq!(created.title, "Untitled Score 1");
        assert_eq!(manager.selected_score_id(), created.id);
        assert_eq!(manager.scores().last().unwrap().id, created.id);
    }

    #[test]
    fn renaming_selected_score_only_updates_that_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(1, "One", "a"),
            record(2, "Two", "b"),
        ]));
        let mut manager = ScoreManager::load(store).unwrap();
        manager.select_score(2);

        manager.rename_selected_score("Renamed").unwrap();

        assert_eq!(manager.scores()[0].title, "One");
        assert_eq!(manager.scores()[1].title, "Renamed");
    }

    #[test]
    fn deleting_selected_score_moves_selection_to_the_next_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![
            record(1, "One", "a"),
            record(2, "Two", "b"),
            record(3, "Three", "c"),
        ]));
        let mut manager = ScoreManager::load(store).unwrap();
        manager.select_score(2);

        manager.delete_selected_score().unwrap();

        assert_eq!(manager.selected_score_id(), 3);
        assert_eq!(manager.scores().len(), 2);
    }

    #[test]
    fn deleting_the_last_score_recreates_a_default_record() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![record(
            1, "Only", "a",
        )]));
        let mut manager = ScoreManager::load(store).unwrap();

        manager.delete_selected_score().unwrap();

        assert_eq!(manager.scores().len(), 1);
        assert_eq!(manager.selected_score().title, DEFAULT_SCORE_TITLE);
    }

    #[test]
    fn updating_selected_source_keeps_the_latest_editor_content() {
        let store = Arc::new(InMemoryScoreStore::with_scores(vec![record(
            1, "One", "old",
        )]));
        let mut manager = ScoreManager::load(store).unwrap();

        manager.update_selected_source("new source").unwrap();

        assert_eq!(manager.selected_score().source, "new source");
    }

    #[test]
    fn sqlite_store_round_trips_scores() {
        let path = unique_test_db_path("round-trip");
        let store = SqliteScoreStore::open(&path).unwrap();

        let created = store.create_score("Round Trip", "source").unwrap();
        let listed = store.list_scores().unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        store.update_score_title(created.id, "Renamed").unwrap();
        store.update_score_source(created.id, "updated").unwrap();

        let updated = store.list_scores().unwrap();
        assert_eq!(updated[0].title, "Renamed");
        assert_eq!(updated[0].source, "updated");

        store.delete_score(created.id).unwrap();
        assert!(store.list_scores().unwrap().is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_initializes_schema_for_an_empty_database() {
        let path = unique_test_db_path("schema");
        let store = SqliteScoreStore::open(&path).unwrap();

        assert!(store.list_scores().unwrap().is_empty());

        let _ = std::fs::remove_file(path);
    }

    fn unique_test_db_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lilypond-studio-{label}-{nanos}.db"))
    }
}
