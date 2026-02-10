use chrono::Utc;

#[derive(Debug, Clone)]
pub struct ForwardedProblem {
    pub id: Option<i64>,
    pub problem_id: String,
    pub status: String,
    pub severity_level: Option<String>,
    pub title: String,
    pub first_seen_at: i64,
    pub last_forwarded_at: i64,
    pub last_status_change_at: i64,
    pub forward_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct ForwardHistory {
    pub id: Option<i64>,
    pub problem_id: String,
    pub connector_name: String,
    pub status: String,
    pub response_code: Option<i32>,
    pub error_message: Option<String>,
    pub forwarded_at: i64,
}

impl ForwardedProblem {
    pub fn new(problem_id: String, status: String, severity_level: Option<String>, title: String) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: None,
            problem_id,
            status,
            severity_level,
            title,
            first_seen_at: now,
            last_forwarded_at: now,
            last_status_change_at: now,
            forward_count: 1,
            created_at: now,
            updated_at: now,
        }
    }
}

impl ForwardHistory {
    pub fn new(
        problem_id: String,
        connector_name: String,
        status: String,
        response_code: Option<i32>,
        error_message: Option<String>,
    ) -> Self {
        Self {
            id: None,
            problem_id,
            connector_name,
            status,
            response_code,
            error_message,
            forwarded_at: Utc::now().timestamp(),
        }
    }
}

#[derive(Debug)]
pub struct DatabaseStats {
    pub total_problems: i64,
    pub open_problems: i64,
    pub closed_problems: i64,
    pub total_forwards: i64,
    pub successful_forwards: i64,
    pub failed_forwards: i64,
}
