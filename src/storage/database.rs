use sqlx::{sqlite::SqlitePool, Pool, Sqlite, Row};
use std::path::Path;
use crate::error::Result;
use crate::storage::models::{ForwardedProblem, ForwardHistory, DatabaseStats};
use chrono::Utc;

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// Create a new database connection
    pub async fn new(db_path: &Path) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let connection_string = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&connection_string).await?;

        let db = Database { pool };

        // Run migrations
        db.run_migrations().await?;

        Ok(db)
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<()> {
        let migration_sql = include_str!("../../migrations/001_initial_schema.sql");
        sqlx::raw_sql(migration_sql).execute(&self.pool).await?;
        Ok(())
    }

    /// Get a forwarded problem by problem_id
    pub async fn get_problem(&self, problem_id: &str) -> Result<Option<ForwardedProblem>> {
        let result = sqlx::query(
            "SELECT id, problem_id, status, severity_level, title, first_seen_at, 
             last_forwarded_at, last_status_change_at, forward_count, created_at, updated_at
             FROM forwarded_problems WHERE problem_id = ?"
        )
        .bind(problem_id)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(ForwardedProblem {
                id: Some(row.get("id")),
                problem_id: row.get("problem_id"),
                status: row.get("status"),
                severity_level: row.get("severity_level"),
                title: row.get("title"),
                first_seen_at: row.get("first_seen_at"),
                last_forwarded_at: row.get("last_forwarded_at"),
                last_status_change_at: row.get("last_status_change_at"),
                forward_count: row.get("forward_count"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    /// Insert a new forwarded problem
    pub async fn insert_problem(&self, problem: &ForwardedProblem) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO forwarded_problems 
             (problem_id, status, severity_level, title, first_seen_at, last_forwarded_at, 
              last_status_change_at, forward_count, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&problem.problem_id)
        .bind(&problem.status)
        .bind(&problem.severity_level)
        .bind(&problem.title)
        .bind(problem.first_seen_at)
        .bind(problem.last_forwarded_at)
        .bind(problem.last_status_change_at)
        .bind(problem.forward_count)
        .bind(problem.created_at)
        .bind(problem.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Update a forwarded problem's status
    pub async fn update_problem_status(&self, problem_id: &str, new_status: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        
        sqlx::query(
            "UPDATE forwarded_problems 
             SET status = ?, last_forwarded_at = ?, last_status_change_at = ?, 
                 forward_count = forward_count + 1, updated_at = ?
             WHERE problem_id = ?"
        )
        .bind(new_status)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(problem_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update last forwarded timestamp (without changing status)
    pub async fn update_last_forwarded(&self, problem_id: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        
        sqlx::query(
            "UPDATE forwarded_problems 
             SET last_forwarded_at = ?, forward_count = forward_count + 1, updated_at = ?
             WHERE problem_id = ?"
        )
        .bind(now)
        .bind(now)
        .bind(problem_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert a forward history record
    pub async fn insert_forward_history(&self, history: &ForwardHistory) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO forward_history 
             (problem_id, connector_name, status, response_code, error_message, forwarded_at)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&history.problem_id)
        .bind(&history.connector_name)
        .bind(&history.status)
        .bind(history.response_code)
        .bind(&history.error_message)
        .bind(history.forwarded_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Clear all forwarded problems (for clear-cache command)
    pub async fn clear_all_problems(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM forwarded_problems")
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<DatabaseStats> {
        let total_problems: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM forwarded_problems")
            .fetch_one(&self.pool)
            .await?;

        let open_problems: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM forwarded_problems WHERE status = 'OPEN'"
        )
        .fetch_one(&self.pool)
        .await?;

        let closed_problems: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM forwarded_problems WHERE status != 'OPEN'"
        )
        .fetch_one(&self.pool)
        .await?;

        let total_forwards: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM forward_history")
            .fetch_one(&self.pool)
            .await?;

        let successful_forwards: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM forward_history WHERE status = 'success'"
        )
        .fetch_one(&self.pool)
        .await?;

        let failed_forwards: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM forward_history WHERE status = 'failed'"
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(DatabaseStats {
            total_problems,
            open_problems,
            closed_problems,
            total_forwards,
            successful_forwards,
            failed_forwards,
        })
    }

    /// Get the connection pool (for testing or advanced usage)
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    /// Close the database connection
    pub async fn close(self) {
        self.pool.close().await;
    }
}
