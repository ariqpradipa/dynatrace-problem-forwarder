use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProblemsResponse {
    #[serde(rename = "totalCount")]
    pub total_count: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
    pub problems: Vec<Problem>,
    #[serde(rename = "nextPageKey")]
    pub next_page_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Problem {
    #[serde(rename = "problemId")]
    pub problem_id: String,
    #[serde(rename = "displayId")]
    pub display_id: String,
    pub title: String,
    #[serde(rename = "impactLevel")]
    pub impact_level: String,
    #[serde(rename = "severityLevel")]
    pub severity_level: String,
    pub status: ProblemStatus,
    #[serde(rename = "affectedEntities")]
    pub affected_entities: Vec<AffectedEntity>,
    #[serde(rename = "impactedEntities")]
    pub impacted_entities: Vec<AffectedEntity>,
    #[serde(rename = "rootCauseEntity")]
    pub root_cause_entity: Option<Entity>,
    #[serde(rename = "managementZones")]
    pub management_zones: Vec<ManagementZone>,
    #[serde(rename = "entityTags")]
    pub entity_tags: Vec<EntityTag>,
    #[serde(rename = "problemFilters")]
    pub problem_filters: Vec<ProblemFilter>,
    #[serde(rename = "startTime")]
    pub start_time: i64,
    #[serde(rename = "endTime")]
    pub end_time: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProblemStatus {
    Open,
    Closed,
    Resolved,
}

impl ToString for ProblemStatus {
    fn to_string(&self) -> String {
        match self {
            ProblemStatus::Open => "OPEN".to_string(),
            ProblemStatus::Closed => "CLOSED".to_string(),
            ProblemStatus::Resolved => "RESOLVED".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AffectedEntity {
    #[serde(rename = "entityId")]
    pub entity_id: EntityId,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entity {
    #[serde(rename = "entityId")]
    pub entity_id: EntityId,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityId {
    pub id: String,
    #[serde(rename = "type")]
    pub entity_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManagementZone {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityTag {
    pub context: String,
    pub key: String,
    pub value: Option<String>,
    #[serde(rename = "stringRepresentation")]
    pub string_representation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProblemFilter {
    pub id: String,
    pub name: String,
}

impl Problem {
    /// Check if the problem is currently open
    pub fn is_open(&self) -> bool {
        self.status == ProblemStatus::Open
    }

    /// Get a summary string for logging
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} - {} ({})",
            self.display_id, self.title, self.status.to_string(), self.severity_level
        )
    }
}
