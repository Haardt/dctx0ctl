//! Language-neutral editable implementation guidance; never execution evidence.
use serde::{Deserialize, Serialize};
pub const STANDARD_PROFILE_SEMANTICS: [&str; 18] = [
    "scalar",
    "value",
    "type",
    "entity",
    "message",
    "event",
    "failure",
    "operation",
    "port",
    "surface",
    "module",
    "external",
    "example",
    "invariant",
    "lifecycle",
    "proof",
    "test",
    "review",
];
pub fn standard_profile_keys() -> Vec<ProfileKey> {
    STANDARD_PROFILE_SEMANTICS
        .into_iter()
        .flat_map(|semantic| {
            [ProfileRole::Implement, ProfileRole::Use].map(|role| ProfileKey {
                role,
                semantic: semantic.into(),
            })
        })
        .collect()
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetManifest {
    pub format_version: u32,
    pub target: String,
    pub template_version: u32,
    pub standard_keys: Vec<ProfileKey>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileRole {
    Implement,
    Use,
}
impl ProfileRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Implement => "implement",
            Self::Use => "use",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileKey {
    pub role: ProfileRole,
    pub semantic: String,
}
impl ProfileKey {
    pub fn path(&self) -> String {
        format!("{}/{}.md", self.role.as_str(), self.semantic)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileTemplate {
    pub path: String,
    pub text: String,
}
pub trait TargetProfileProvider {
    fn manifest(&self) -> TargetManifest;
    fn templates(&self) -> Vec<ProfileTemplate>;
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocument {
    pub target: String,
    pub key: Option<ProfileKey>,
    pub path: String,
    pub text: String,
    pub content_hash: String,
    pub subjects: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionBundle {
    pub target: String,
    pub documents: Vec<ProfileDocument>,
    pub hash: String,
}
